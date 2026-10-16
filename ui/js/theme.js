// Applied before first paint so dark mode never flashes white.
(function () {
  var t = null;
  try { t = localStorage.getItem("theme"); } catch (e) {}
  if (!t) t = "dark"; // the design kit is dark-first; the sidebar button switches to light
  document.documentElement.setAttribute("data-theme", t);
})();
