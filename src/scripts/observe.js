(condition => {
  let el;
  try { el = condition.selector ? document.querySelector(condition.selector) : document.body; }
  catch (_) { return {invalid_selector:true}; }
  const exists = !!el;
  const text = el ? String(/^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) ? el.value : el.innerText || el.textContent || '') : '';
  return {exists,text};
})
