export const pad2 = (n: number) => String(n).padStart(2, "0");
const hex4 =(byteAddress: number) => `0x${byteAddress.toString(16).toUpperCase().padStart(4, "0")}`;
/** Byte address of the i32 cell at `index`. */
export const cellAddress = (index: number) => hex4(index * 4);
/** The value shown for a cell: the allocator pointer (cell 0) is an address, so it reads in hex like every other address; 0 means nothing allocated yet. */
export const cellValue = (index: number, value: number) => (index === 0 && value > 0 ? hex4(value) : String(value));
export const isMac = () => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
