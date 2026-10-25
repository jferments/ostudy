// Runs on the audio thread: hands the microphone's first channel to the page in
// blocks of about 0.1 s. Plain JavaScript because worklet modules load as is.
const BLOCK = 2048;

class CaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.block = new Float32Array(BLOCK);
    this.filled = 0;
    // Stopping asks for the part of a block recorded since the last one.
    this.port.onmessage = () => {
      this.port.postMessage({ last: this.block.slice(0, this.filled) });
      this.filled = 0;
    };
  }

  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel) return true;
    let offset = 0;
    while (offset < channel.length) {
      const count = Math.min(channel.length - offset, BLOCK - this.filled);
      this.block.set(channel.subarray(offset, offset + count), this.filled);
      this.filled += count;
      offset += count;
      if (this.filled === BLOCK) {
        this.port.postMessage(this.block);
        this.block = new Float32Array(BLOCK);
        this.filled = 0;
      }
    }
    return true;
  }
}

registerProcessor("ostudy-microphone-capture", CaptureProcessor);
