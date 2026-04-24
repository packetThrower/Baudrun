// Streaming hex-dump formatter. Accepts arbitrary byte chunks and emits
// fixed-width lines of the form:
//
//   00000000  48 65 6c 6c 6f 20 57 6f  72 6c 64 0d 0a           |Hello World..|
//
// Lines are 16 bytes wide with a gap after 8 bytes. Partial lines are held
// in a buffer and flushed after a short idle period so prompts and small
// reads appear quickly.

const BYTES_PER_LINE = 16;

export class HexFormatter {
  private offset = 0;
  private buffer: number[] = [];
  private flushTimer: number | null = null;
  private flushMs = 100;

  constructor(private writeCb: (text: string) => void) {}

  feed(bytes: Uint8Array) {
    for (let i = 0; i < bytes.length; i++) {
      this.buffer.push(bytes[i]);
      if (this.buffer.length >= BYTES_PER_LINE) {
        this.emitLine(this.buffer.splice(0, BYTES_PER_LINE));
      }
    }
    this.scheduleFlush();
  }

  private emitLine(bytes: number[]) {
    const offsetStr = this.offset.toString(16).padStart(8, "0");
    const hexParts: string[] = [];
    for (let i = 0; i < BYTES_PER_LINE; i++) {
      if (i === 8) hexParts.push(""); // extra space after 8 bytes
      if (i < bytes.length) {
        hexParts.push(bytes[i].toString(16).padStart(2, "0"));
      } else {
        hexParts.push("  ");
      }
    }
    const ascii = bytes
      .map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : "."))
      .join("");
    this.writeCb(`${offsetStr}  ${hexParts.join(" ")}  |${ascii}|\r\n`);
    this.offset += bytes.length;
  }

  private scheduleFlush() {
    if (this.flushTimer !== null) {
      window.clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    if (this.buffer.length === 0) return;
    this.flushTimer = window.setTimeout(() => this.flushPartial(), this.flushMs);
  }

  private flushPartial() {
    if (this.flushTimer !== null) {
      window.clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }
    if (this.buffer.length > 0) {
      this.emitLine(this.buffer.splice(0, this.buffer.length));
    }
  }

  reset() {
    this.flushPartial();
    this.offset = 0;
    this.buffer = [];
  }

  dispose() {
    this.flushPartial();
  }
}
