// A feed row's second line (its sub and its pills): whether it holds anything to see. The time is pinned in the row's
// corner, so a pills' cell with only the time says nothing on that line. Pure DOM reads, no layout.

const shows = (el: Element): boolean => (el.textContent ?? '').trim() !== '' || el.querySelector('svg, img') !== null || el.matches('svg, img');

export function secondLineEmpty(sub: Element, pills: Element): boolean {
  if (shows(sub)) return false;
  for (const c of pills.children) if (c.getAttribute('data-kit') !== 'time' && !c.matches('[data-part~="list.row.time"]') && shows(c)) return false;
  return true;
}
