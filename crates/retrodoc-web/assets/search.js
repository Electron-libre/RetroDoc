// The search box as a combobox over its hits panel (specs/web-ui.md, WEB-110 to WEB-112): `/` puts
// the focus in the box from any page, outside a field; in the box, the arrows move the active hit,
// Enter follows it (with none, the box is submitted to the search page), Escape and a click outside
// close the panel. htmx fills the panel as the query is typed (WEB-39).
(function () {
  "use strict";

  const box = document.querySelector("form.search input[role=combobox]");
  const panel = document.getElementById("search-panel");
  if (!box || !panel) {
    return;
  }
  let active = -1;

  function options() {
    return Array.from(panel.querySelectorAll("[role=option]"));
  }

  function open() {
    return !panel.hidden && options().length > 0;
  }

  function setActive(index) {
    const all = options();
    active = index;
    all.forEach(function (option, i) {
      if (i === index) {
        option.setAttribute("aria-selected", "true");
        option.scrollIntoView({ block: "nearest" });
      } else {
        option.removeAttribute("aria-selected");
      }
    });
    if (index >= 0 && all[index]) {
      box.setAttribute("aria-activedescendant", all[index].id);
    } else {
      box.removeAttribute("aria-activedescendant");
    }
  }

  function show(shown) {
    panel.hidden = !shown;
    box.setAttribute("aria-expanded", String(shown && options().length > 0));
    if (!shown) {
      setActive(-1);
    }
  }

  // A new answer: shown, no hit active yet; an empty one closes the panel.
  document.addEventListener("htmx:afterSwap", function (event) {
    if (event.detail.target === panel) {
      active = -1;
      show(panel.children.length > 0);
    }
  });

  function inField(target) {
    return (
      target.isContentEditable ||
      ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)
    );
  }

  document.addEventListener("keydown", function (event) {
    if (event.key !== "/" || event.ctrlKey || event.metaKey || event.altKey || inField(event.target)) {
      return;
    }
    event.preventDefault();
    box.focus();
    box.select();
  });

  box.addEventListener("keydown", function (event) {
    if (event.isComposing) {
      return;
    }
    const count = options().length;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      if (count === 0) {
        return;
      }
      event.preventDefault();
      if (!open()) {
        show(true);
      }
      const step = event.key === "ArrowDown" ? 1 : -1;
      const from = active < 0 ? (step > 0 ? -1 : count) : active;
      setActive((from + step + count) % count);
    } else if (event.key === "Enter") {
      const option = open() && active >= 0 ? options()[active] : null;
      if (option) {
        event.preventDefault();
        if (option.href) {
          window.location.assign(option.href);
        }
      }
    } else if (event.key === "Escape") {
      if (open()) {
        // Kept from the box's own Escape, which would empty it.
        event.preventDefault();
        show(false);
      }
    }
  });

  document.addEventListener("click", function (event) {
    if (open() && !box.form.contains(event.target)) {
      show(false);
    }
  });
})();
