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
