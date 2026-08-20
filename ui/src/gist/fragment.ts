import { populateDates } from '../explorer/explorer';

// Fetch a gist page as an htmx-style fragment and swap the live article for
// the one the response carries: request with HX-Request headers, parse the
// HTML, select the replacement, let the caller prepare it, then swap and
// re-run the post-swap lifecycle (dates, content-ready listeners). Returns
// the inserted article, or null when the fetch or the fragment shape fails.
export async function swapArticle(
  article: Element,
  path: string,
  selector: string,
  prepare?: (next: HTMLElement) => void,
): Promise<HTMLElement | null> {
  const response = await fetch(path, {
    headers: {
      Accept: 'text/html',
      'HX-Request': 'true',
    },
  });
  if (!response.ok) return null;

  const html = await response.text();
  const doc = new DOMParser().parseFromString(html, 'text/html');
  const next = doc.querySelector<HTMLElement>(selector);
  if (!next) return null;

  prepare?.(next);
  article.replaceWith(next);
  populateDates();
  document.dispatchEvent(new CustomEvent('ghrm:contentready'));
  return next;
}
