// DOM-derived roles/names, not a complete platform accessibility tree.
(options => {
  const norm = s => String(s || '').replace(/\s+/g, ' ').trim();
  const name = el => {
    const ids = el.getAttribute('aria-labelledby');
    if (ids) {
      const value = norm(ids.split(/\s+/).map(id => document.getElementById(id)?.textContent || '').join(' '));
      if (value) return value;
    }
    return norm(el.getAttribute('aria-label') ||
      (el.labels?.length ? Array.from(el.labels).map(l => l.textContent).join(' ') : '') ||
      (el.tagName === 'IMG' ? el.getAttribute('alt') : '') ||
      (el.tagName === 'INPUT' && /^(submit|button|reset)$/.test(el.type) ? el.value : '') ||
      el.innerText || el.getAttribute('title') || el.getAttribute('placeholder'));
  };
  const role = el => el.getAttribute('role')?.split(/\s+/)[0] || ({
    BUTTON: 'button', A: el.hasAttribute('href') ? 'link' : '', TEXTAREA: 'textbox',
    SELECT: el.multiple || el.size > 1 ? 'listbox' : 'combobox', SUMMARY: 'button',
    INPUT: ({checkbox:'checkbox',radio:'radio',range:'slider',number:'spinbutton',
      button:'button',submit:'button',reset:'button',search:'searchbox'})[el.type] || 'textbox'
  })[el.tagName] || (el.isContentEditable ? 'textbox' : '');
  const visible = el => {
    if (el.closest('[hidden],[inert],[aria-hidden="true"]')) return false;
    const style = getComputedStyle(el);
    return style.display !== 'none' && style.visibility !== 'hidden' &&
      style.visibility !== 'collapse' && el.getClientRects().length > 0;
  };
  let counter = Number(window.__ffrdp_refs) || 0;
  for (const el of document.querySelectorAll('[data-ffrdp-ref]'))
    counter = Math.max(counter, Number(el.getAttribute('data-ffrdp-ref').slice(1)) || 0);
  if (!counter) counter = (100 + Math.floor(Math.random()*900))*1000;
  const seen = new Set();
  const stamp = el => {
    let ref = el.getAttribute('data-ffrdp-ref');
    if (!/^e\d+$/.test(ref || '') || seen.has(ref) || document.querySelectorAll('[data-ffrdp-ref="' + ref + '"]').length > 1) {
      ref = 'e' + (++counter);
      el.setAttribute('data-ffrdp-ref', ref);
    }
    seen.add(ref);
    return ref;
  };
  const rows = [];
  for (const el of document.querySelectorAll('a[href],button,input:not([type=hidden]),textarea,select,summary,[role],[contenteditable=true]')) {
    if (!visible(el)) continue;
    const r = role(el), n = name(el);
    if (!r) continue;
    if (options.role && r !== options.role) continue;
    if (options.name !== undefined && (options.exact !== false ? n !== norm(options.name) : !n.toLocaleLowerCase().includes(norm(options.name).toLocaleLowerCase()))) continue;
    if (options.query && !n.toLocaleLowerCase().includes(norm(options.query).toLocaleLowerCase())) continue;
    const zone = el.closest('dialog,[role=dialog],[role=alertdialog]') ? 'dialog' :
      el.closest('main,[role=main]') ? 'content' :
      el.closest('header,footer,nav,[role=banner],[role=contentinfo],[role=navigation]') ? 'chrome' : 'content';
    const row = {ref:stamp(el),role:r,name:n.length > 500 ? n.slice(0,500) : n,zone,
      disabled:!!el.disabled || el.getAttribute('aria-disabled') === 'true'};
    if (n.length > 500) row.name_truncated = true;
    if (el.href) row.href = el.href;
    if ('checked' in el) row.checked = el.checked;
    if (el.hasAttribute('aria-expanded')) row.expanded = el.getAttribute('aria-expanded') === 'true';
    rows.push(row);
  }
  window.__ffrdp_refs = counter;
  rows.sort((a,b) => ({dialog:0,content:1,chrome:2})[a.zone] - ({dialog:0,content:1,chrome:2})[b.zone]);
  return {source:'dom-semantics',title:document.title,url:location.href,
    elements:rows,total:rows.length,
    hint:'Use refs with click/type. Narrow query or name if elements are omitted. Roles/names are DOM-derived; use format=tree for backend snapshot. Main document only.'};
})
