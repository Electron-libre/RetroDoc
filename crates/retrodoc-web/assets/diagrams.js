// Draws the diagrams of the page (`<pre class="mermaid">`). One folded in a
// closed `<details>` is drawn when it first opens: drawn hidden, it would
// measure nothing. Their colours are those of the stylesheet's charter: boxes
// on the raised surface, lines and borders muted, gold kept for the notes.
const colour = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
const [surface, raised, border, text, muted, goldDark] =
  ["--surface", "--surface-2", "--border", "--text", "--text-muted", "--gold-dark"].map(colour);
mermaid.initialize({
  startOnLoad: false,
  securityLevel: "strict",
  theme: "base",
  themeVariables: {
    darkMode: true,
    fontFamily: getComputedStyle(document.body).fontFamily,
    background: surface,
    primaryColor: raised,
    primaryTextColor: text,
    primaryBorderColor: muted,
    secondaryColor: surface,
    tertiaryColor: surface,
    lineColor: muted,
    textColor: text,
    clusterBkg: surface,
    clusterBorder: border,
    edgeLabelBackground: surface,
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
