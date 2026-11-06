// The composer of the ask page (specs/web-ask.md): Enter sends the question, Shift+Enter breaks
// its line (WAK-41); the field grows with its text where the browser has no `field-sizing`, the
// stylesheet capping it at 12 lines (WAK-42); once the answer of a question replaces its running
// exchange, the field gets the focus back and the exchange is scrolled into view (WAK-45); the
// button to start over shows once the conversation holds an exchange (WAK-46); a suggested question
// fills the field without sending it (WAK-44), and the suggestions go once a question is sent.
(function () {
  "use strict";

  const sizesItself = window.CSS && CSS.supports("field-sizing", "content");

  function field() {
    return document.querySelector("form.ask textarea");
  }

  function grow(area) {
    if (sizesItself || !area) {
      return;
    }
    area.style.height = "auto";
    area.style.height = area.scrollHeight + 2 + "px";
  }

  document.addEventListener("keydown", function (event) {
    const area = event.target;
    if (!(area instanceof HTMLTextAreaElement) || !area.matches("form.ask textarea")) {
      return;
    }
    // An input method composing a word takes Enter for itself (keyCode 229 in older browsers).
    if (event.key !== "Enter" || event.shiftKey || event.isComposing || event.keyCode === 229) {
      return;
    }
    event.preventDefault();
    const button = area.form.querySelector("button[type=submit]");
    if (button && button.disabled) {
      return;
    }
    // Through the submit event, so htmx sends it and the browser checks the field first.
    area.form.requestSubmit();
  });

  document.addEventListener("click", function (event) {
    const suggestion = event.target instanceof Element && event.target.closest("button.suggestion");
    const area = field();
    if (!suggestion || !area) {
      return;
    }
    area.value = suggestion.getAttribute("data-question");
    grow(area);
    area.focus();
    area.setSelectionRange(area.value.length, area.value.length);
  });

  document.addEventListener("input", function (event) {
    if (event.target instanceof HTMLTextAreaElement && event.target.matches("form.ask textarea")) {
      grow(event.target);
    }
  });

  // The form resets once its question is sent: the field shrinks back.
  document.addEventListener("htmx:afterRequest", function () {
    grow(field());
  });

  let waiting = false;

  function follow() {
    const exchanges = document.getElementById("exchanges");
    if (!exchanges) {
      return;
    }
    const reset = document.querySelector("form.ask-reset");
    if (exchanges.querySelector("article.exchange")) {
      if (reset) {
        reset.hidden = false;
      }
      const suggestions = document.querySelector("ul.suggestions");
      if (suggestions) {
        suggestions.remove();
      }
    }
    if (exchanges.querySelector(".running")) {
      waiting = true;
      return;
    }
    if (!waiting) {
      return;
    }
    waiting = false;
    const answered = exchanges.lastElementChild;
    if (answered) {
      answered.scrollIntoView({ block: "start", behavior: "smooth" });
    }
    const area = field();
    if (area) {
      area.focus({ preventScroll: true });
    }
  }

  document.addEventListener("htmx:afterSettle", follow);
  document.addEventListener("DOMContentLoaded", function () {
    grow(field());
    follow();
  });
})();
