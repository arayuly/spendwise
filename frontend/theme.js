(function () {
  function applyTheme() {
    var preference = 'system';
    try { preference = localStorage.getItem('spendwise-theme') || 'system'; } catch (_) {}
    var dark = preference === 'dark' || (preference === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
    document.documentElement.classList.toggle('dark', dark);
    document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
  }
  applyTheme();
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);
  window.addEventListener('spendwise-theme', applyTheme);
})();
