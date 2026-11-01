// Draws the diagrams of the page (`<pre class="mermaid">`). One folded in a
// closed `<details>` is drawn when it first opens: drawn hidden, it would
// measure nothing.
mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
window.addEventListener("load", () => {
  for (const pre of document.querySelectorAll("pre.mermaid")) {
    const folded = pre.closest("details:not([open])");
    if (folded) {
      folded.addEventListener("toggle", () => mermaid.run({ nodes: [pre] }), { once: true });
    } else {
      mermaid.run({ nodes: [pre] });
    }
  }
});
