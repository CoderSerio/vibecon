export class RingBuffer<T> {
  private readonly storage: Array<T | undefined>;
  private writeIndex = 0;
  private count = 0;

  constructor(readonly capacity: number) {
    if (!Number.isInteger(capacity) || capacity <= 0) {
      throw new RangeError("RingBuffer capacity must be a positive integer");
    }
    this.storage = new Array<T | undefined>(capacity);
  }

  get size() {
    return this.count;
  }

  push(value: T) {
    this.storage[this.writeIndex] = value;
    this.writeIndex = (this.writeIndex + 1) % this.capacity;
    this.count = Math.min(this.count + 1, this.capacity);
  }

  *newestFirst(): IterableIterator<T> {
    for (let offset = 0; offset < this.count; offset += 1) {
      const index = (this.writeIndex - 1 - offset + this.capacity) % this.capacity;
      const value = this.storage[index];
      if (value !== undefined) yield value;
    }
  }

  clear() {
    this.storage.fill(undefined);
    this.writeIndex = 0;
    this.count = 0;
  }
}
