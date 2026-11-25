export const pad2 = (n: number) => String(n).padStart(2, "0");
const hex4 =(byteAddress: number) => `0x${byteAddress.toString(16).toUpperCase().padStart(4, "0")}`;
/** Byte address of the i32 cell at `index`. */
export const cellAddress = (index: number) => hex4(index * 4);
export const isMac = () => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
