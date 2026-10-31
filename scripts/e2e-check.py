# /// script
# requires-python = ">=3.9"
# dependencies = ["pyserial>=3.5"]
# ///
"""Run the end-to-end firmware (firmware/e2e) on a board and check it.

Resets the board, reads its report, and for every message re-computes
the ciphertext and tag with an independent Ascon-AEAD128 implementation
(the ascon-c reference, through tools/differential/ascon_c_driver.c in
the ASCON-IoT repository). Repeats over several resets and checks that
every reset produced a different key and nonce.

  uv run scripts/e2e-check.py --port COM7 --ascon-c ./ascon_c_driver --runs 3
  uv run scripts/e2e-check.py --port COM6 --reset break --ascon-c ./ascon_c_driver

Reset: `dtr` (default) toggles DTR, which auto-resets an Arduino;
`break` sends a serial break, which makes the nRF52840 MDK's DAPLink
reset the target; `rts` pulses RTS with DTR released, for ESP32 boards. Exit status 0 only if every check passed.
"""

import argparse
import subprocess
import sys
import time

import serial


def reset(ser, how):
    ser.reset_input_buffer()
    if how == "dtr":
        ser.dtr = False
        time.sleep(0.1)
        ser.reset_input_buffer()
        ser.dtr = True
    elif how == "rts":
        ser.dtr = False
        ser.rts = True
        time.sleep(0.1)
        ser.reset_input_buffer()
        ser.rts = False
    else:
        ser.send_break(0.25)


def read_report(ser, timeout):
    lines, buf, deadline = [], b"", time.monotonic() + timeout
    while time.monotonic() < deadline:
        buf += ser.read(ser.in_waiting or 1)
        while b"\n" in buf:
            raw, buf = buf.split(b"\n", 1)
            line = raw.decode("ascii", "replace").strip()
            if not lines and not line.startswith("QPPE2E"):
                continue  # boot noise before the header
            lines.append(line)
            if line in ("DONE", "PANIC"):
                return lines
    lines.append("TIMEOUT")
    return lines


def fields(line):
    out = {}
    for tok in line.split():
        if "=" in tok:
            k, v = tok.split("=", 1)
            out[k] = v
    return out


def ascon_c(driver, key, nonce, ad, pt):
    out = subprocess.run(driver + ["enc", key, nonce, ad, pt], capture_output=True, text=True, check=True).stdout
    d = fields(out.replace("\n", " "))
    return d.get("CT", ""), d["TAG"]


def check_run(lines, driver):
    """Returns (summary dict, list of problems)."""
    problems = []
    if lines[-1] != "DONE":
        return {}, [f"report ended with {lines[-1]}"]
    head = fields(lines[0])
    keyl = fields(lines[1])
    msgs = [fields(l) for l in lines if l.startswith("MSG ")]
    tail = fields(lines[-2])
    hz = int(head["cpu_hz"])
    if len(msgs) != 3:
        problems.append(f"expected 3 messages, got {len(msgs)}")
    for m in msgs:
        ct, tag = ascon_c(driver, keyl["KEY"], m["NONCE"], m["AD"], m["PT"])
        if (ct, tag) != (m.get("CT", "").upper(), m["TAG"].upper()):
            problems.append(f"len={m['len']}: board CT/TAG differ from ascon-c")
        if m.get("DEC") != "ok":
            problems.append(f"len={m['len']}: decryption failed")
        if m.get("TAMPER") != "rejected":
            problems.append(f"len={m['len']}: tampered message accepted")
    if tail.get("RESULT") != "PASS":
        problems.append("firmware reported RESULT=FAIL")
    summary = {
        "chip": head["chip"],
        "key": keyl["KEY"],
        "nonce": keyl["NONCE"],
        "key_ms": int(keyl["T_KEY"]) * 1e3 / hz,
        "nonce_ms": int(keyl["T_NONCE"]) * 1e3 / hz,
        "init_ms": int(keyl["T_INIT"]) * 1e3 / hz,
        "msgs": [(int(m["len"]), int(m["T_ENC"]) * 1e3 / hz, int(m["T_DEC"]) * 1e3 / hz) for m in msgs],
        "stack_peak": int(tail["STACK_PEAK"]),
        "stack_headroom": int(tail["STACK_HEADROOM"]),
        "statics_end": int(head["statics_end"]),
        "ram_top": int(head["ram_top"]),
        "verified": len(msgs) - sum(1 for p in problems if p.startswith("len=") and "ascon-c" in p),
    }
    return summary, problems


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", required=True)
    ap.add_argument("--reset", choices=["dtr", "break", "rts"], default="dtr")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--timeout", type=float, default=90.0, help="seconds per run")
    ap.add_argument("--ascon-c", required=True, nargs="+", help="ascon_c_driver command (may be several words)")
    ap.add_argument("--log", help="write every report here")
    args = ap.parse_args()

    ok, seen, log = True, set(), []
    with serial.Serial(args.port, 115200, timeout=0.2) as ser:
        for run in range(1, args.runs + 1):
            reset(ser, args.reset)
            lines = read_report(ser, args.timeout)
            log += [f"# run {run}"] + lines
            summary, problems = check_run(lines, args.ascon_c)
            if summary:
                pair = (summary["key"], summary["nonce"])
                if pair in seen:
                    problems.append("key/nonce repeated from an earlier reset")
                seen.add(pair)
                msgs = ", ".join(f"{n} B enc {e:.2f} ms / dec {d:.2f} ms" for n, e, d in summary["msgs"])
                print(f"run {run} [{summary['chip']}]: key {summary['key_ms']:.0f} ms, nonce {summary['nonce_ms']:.2f} ms, "
                      f"cipher init {summary['init_ms']:.2f} ms; {msgs}; "
                      f"ascon-c agrees on {summary['verified']}/{len(summary['msgs'])}; "
                      f"stack peak {summary['stack_peak']} B, untouched {summary['stack_headroom']} B")
                print(f"        key {summary['key']} nonce {summary['nonce']}")
            for p in problems:
                print(f"run {run}: PROBLEM: {p}")
            ok &= not problems
    if args.log:
        with open(args.log, "w") as f:
            f.write("\n".join(log) + "\n")
    print("ALL CHECKS PASSED" if ok else "SOME CHECKS FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
