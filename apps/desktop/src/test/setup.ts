import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(() => {
  cleanup();
});

// jsdom has no layout; give the virtualiser a viewport to measure.
Object.defineProperty(HTMLElement.prototype, "offsetHeight", { configurable: true, value: 600 });
Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, value: 600 });
Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, value: 800 });
Object.defineProperty(HTMLElement.prototype, "offsetWidth", { configurable: true, value: 800 });
Element.prototype.getBoundingClientRect = () => ({
  width: 800,
  height: 600,
  top: 0,
  left: 0,
  right: 800,
  bottom: 600,
  x: 0,
  y: 0,
  toJSON() {},
});
class RO {
  constructor(private cb: ResizeObserverCallback) {}
  observe(target: Element) {
    const box = [{ inlineSize: 800, blockSize: 600 }];
    const entry = {
      target,
      contentRect: target.getBoundingClientRect(),
      borderBoxSize: box,
      contentBoxSize: box,
    } as unknown as ResizeObserverEntry;
    this.cb([entry], this as unknown as ResizeObserver);
  }
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = RO as unknown as typeof ResizeObserver;
window.ResizeObserver = RO as unknown as typeof ResizeObserver;
Element.prototype.scrollIntoView = () => {};
