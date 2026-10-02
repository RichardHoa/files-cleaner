/** `el("p", { className: "x" }, "text", child)`: builds an element. */
export function el(tag, props = {}, ...children) {
  const node = Object.assign(document.createElement(tag), props);
  node.append(...children);
  return node;
}
