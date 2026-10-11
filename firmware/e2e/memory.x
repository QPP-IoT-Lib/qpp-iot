/* nRF52840 (makerdiary nRF52840 MDK), no bootloader or SoftDevice. */
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 1024K
  RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}
