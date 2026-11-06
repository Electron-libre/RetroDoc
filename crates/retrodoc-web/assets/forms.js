// The answers of the forms sent with htmx (specs/web-ui.md, WEB-106): htmx swaps nothing of an
// error answer by default. An HTML one is the form again, the error in it and the fields as sent:
// it is swapped. Any other (a refusal in plain text, a server that failed) is shown in the notice
// of the page, the form left as it was. A page asked after a save (`?saved=review`, WEB-104) says
// so once: the parameter is dropped from the address bar, so a reload doesn't say it again.
(function () {
  "use strict";

  const url = new URL(window.location.href);
  if (url.searchParams.has("saved")) {
    url.searchParams.delete("saved");
    window.history.replaceState(window.history.state, "", url.pathname + url.search + url.hash);
  }

  function isHtml(xhr) {
    return (xhr.getResponseHeader("Content-Type") || "").startsWith("text/html");
  }

  document.addEventListener("htmx:beforeSwap", function (event) {
    if (event.detail.isError && isHtml(event.detail.xhr)) {
      // Swapped, still an error: the request stays failed, so a form that resets on success
      // keeps what was typed.
      event.detail.shouldSwap = true;
    }
  });

  document.addEventListener("htmx:beforeRequest", function () {
    const notice = document.getElementById("page-error");
    if (notice) {
      notice.hidden = true;
    }
  });

  document.addEventListener("htmx:responseError", function (event) {
    const xhr = event.detail.xhr;
    const notice = document.getElementById("page-error");
    if (isHtml(xhr) || !notice) {
      return;
    }
    const text = (xhr.responseText || "").trim().slice(0, 300);
    notice.textContent = xhr.status + " " + xhr.statusText + (text ? ": " + text : "");
    notice.hidden = false;
  });
})();
