// Keep startup independent of React and optimized vendor modules. A failed
// import (including a stale development chunk) must leave a recoverable view.
const recovery = document.getElementById('startup-recovery');
const retry = document.getElementById('startup-retry');
const message = document.getElementById('startup-message');

retry?.addEventListener('click', () => window.location.reload());

function failed() {
  if (!recovery || !message || !retry) return;
  message.textContent = 'Jacket couldn’t open. Please reload to try again.';
  recovery.setAttribute('role', 'alert');
  retry.hidden = false;
}

window.addEventListener('vite:preloadError', (event) => {
  event.preventDefault();
  failed();
});

import('./main').catch((error: unknown) => {
  console.error('Jacket startup failed', error);
  failed();
});
