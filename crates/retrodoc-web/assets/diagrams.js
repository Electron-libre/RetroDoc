// Draws the diagrams of the page (`<pre class="mermaid">`). One folded in a
// closed `<details>` is drawn when it first opens: drawn hidden, it would
// measure nothing. Their colours are those of the charter of `style.css`,
// written here rather than read from its variables: a stylesheet still cached
// from an older binary would leave them empty and `mermaid` would throw.
const charter = {
  bg: "#0e0e10",
  raised: "#1f1f25",
  border: "#2a2a31",
  text: "#e8e6e1",
  muted: "#9b978e",
  goldDark: "#8f7328",
};
const { bg, raised, border, text, muted, goldDark } = charter;
mermaid.initialize({
  startOnLoad: false,
  securityLevel: "strict",
  theme: "base",
  themeVariables: {
    darkMode: true,
    fontFamily: getComputedStyle(document.body).fontFamily,
    background: bg,
    primaryColor: raised,
    primaryTextColor: text,
    primaryBorderColor: muted,
    secondaryColor: bg,
    tertiaryColor: bg,
    lineColor: muted,
    textColor: text,
    clusterBkg: bg,
    clusterBorder: border,
    edgeLabelBackground: bg,
    actorBkg: raised,
    actorBorder: muted,
    actorTextColor: text,
    actorLineColor: muted,
    signalColor: text,
    signalTextColor: text,
    labelBoxBkgColor: raised,
    labelBoxBorderColor: muted,
    labelTextColor: text,
    loopTextColor: text,
    noteBkgColor: raised,
    noteBorderColor: goldDark,
    noteTextColor: text,
    activationBkgColor: raised,
    activationBorderColor: muted,
  },
});
// A map keeps beside its graph the list of its items, each link carrying the
// id of its node (`data-node`): the drawn node opens that link. `mermaid`
// names a node `flowchart-<id>-<n>`; `securityLevel: "strict"` refuses the
// `click` of the source, hence this.
function linkNodes(pre) {
  const map = pre.closest(".map");
  const svg = map && pre.querySelector("svg");
  if (!svg) return;
  for (const link of map.querySelectorAll("a[data-node]")) {
    const node = svg.querySelector(`g.node[id*="flowchart-${link.dataset.node}-"]`);
    if (!node) continue;
    node.classList.add("linked");
    node.setAttribute("role", "link");
    node.setAttribute("tabindex", "0");
    node.setAttribute("aria-label", link.textContent);
    const open = () => { window.location.href = link.href; };
    node.addEventListener("click", open);
    node.addEventListener("keydown", (event) => {
      if (event.key === "Enter") open();
    });
  }
}
function draw(pre) {
  mermaid.run({ nodes: [pre] }).then(() => linkNodes(pre));
}
window.addEventListener("load", () => {
  for (const pre of document.querySelectorAll("pre.mermaid")) {
    const folded = pre.closest("details:not([open])");
    if (folded) {
      folded.addEventListener("toggle", () => draw(pre), { once: true });
    } else {
      draw(pre);
    }
  }
});
