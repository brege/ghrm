document.addEventListener('DOMContentLoaded', function() {
  var status = document.getElementById('js-status');
  if (status) {
    status.textContent = 'LOADED';
    status.classList.add('loaded');
  }
});
