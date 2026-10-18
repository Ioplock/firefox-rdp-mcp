// Offline unit fixtures for the fixed page scripts. Real DOM integration is
// covered separately by browser_smoke.py against an existing Firefox.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const root = process.argv[2] || path.join(__dirname, '..', 'src', 'scripts');
const script = name => fs.readFileSync(path.join(root, name + '.js'), 'utf8');
const nodes = [];
const document = {
  title:'Fixture', body:{innerText:'Cart 1'},
  getElementById: id => nodes.find(n => n.attrs.id === id),
  querySelectorAll(selector) {
    if (selector === '[data-ffrdp-ref]') return nodes.filter(n => n.hasAttribute('data-ffrdp-ref'));
    if (selector.startsWith('[data-ffrdp-ref="')) {
      const ref = selector.slice(17, -2);
      return nodes.filter(n => n.attrs['data-ffrdp-ref'] === ref);
    }
    return nodes;
  },
  querySelector(selector) {
    if (selector.includes(':contains(')) throw new SyntaxError('invalid CSS');
    if (selector.startsWith('#')) return this.getElementById(selector.slice(1)) || null;
    return null;
  }
};
function element(tag, text, attrs = {}, zone = 'content') {
  const el = {
    tagName:tag, innerText:text, textContent:text, attrs:{...attrs}, zone,
    getAttribute(k) { return this.attrs[k] ?? null; },
    setAttribute(k,v) { this.attrs[k] = v; },
    hasAttribute(k) { return Object.hasOwn(this.attrs,k); },
    getClientRects() { return [{}]; },
    closest(selector) {
      if (selector.includes('[hidden]')) return this.attrs.hidden ? this : null;
      if (selector.startsWith('dialog')) return this.zone === 'dialog' ? this : null;
      if (selector.startsWith('main')) return this.zone === 'content' ? this : null;
      return this.zone === 'chrome' ? this : null;
    }
  };
  nodes.push(el);
  return el;
}
const window = {};
const context = {document,window,location:{href:'https://fixture.test/'},
  getComputedStyle: () => ({display:'block',visibility:'visible'})};
const elements = options => vm.runInNewContext('(' + script('elements') + ')(' + JSON.stringify(options) + ')',context);
element('BUTTON','Header action',{},'chrome');
const buy = element('BUTTON','  Add   to cart ',{'data-ffrdp-ref':'e1',id:'buy'});
buy.value = ''; // Buttons have a value too, but observation must read their text.
const duplicate = element('BUTTON','Add to cart',{'data-ffrdp-ref':'e1'});
element('BUTTON','Hidden',{hidden:true});
element('BUTTON','Dialog action',{},'dialog');
const input = element('INPUT','',{id:'quantity'});
input.type = 'number'; input.value = '1'; input.labels = [{textContent:'Quantity'}];
element('SPAN','Named by label',{id:'label'});
element('BUTTON','Other text',{'aria-labelledby':'label'});
const link = element('A','Details',{href:'/item'}); link.href = 'https://fixture.test/item?keep=yes';
let result = elements({role:'button',name:'Add to cart'});
assert.equal(result.total,2);
assert.notEqual(result.elements[0].ref,result.elements[1].ref);
assert.equal(document.querySelectorAll('[data-ffrdp-ref="'+buy.attrs['data-ffrdp-ref']+'"]').length,1);
assert.notEqual(buy.attrs['data-ffrdp-ref'],duplicate.attrs['data-ffrdp-ref']);
assert.equal(elements({role:'button',name:'ADD TO',exact:false}).total,2);
assert.equal(elements({role:'button',name:'Hidden'}).total,0);
assert.equal(elements({role:'spinbutton',name:'Quantity'}).total,1);
assert.equal(elements({name:'Named by label'}).total,1);
result = elements({});
assert.equal(result.elements[0].zone,'dialog');
assert.equal(result.elements.at(-1).zone,'chrome');
assert.equal(result.elements.find(x => x.role === 'link').href,link.href);
assert.equal(elements({query:'Add to cart'}).total,2);
const observe = condition => vm.runInNewContext('(' + script('observe') + ')(' + JSON.stringify(condition) + ')',context);
assert.equal(observe({selector:'#quantity'}).text,'1');
assert.equal(observe({selector:'#buy'}).text,buy.innerText);
assert.equal(observe({selector:'#missing'}).exists,false);
assert.equal(observe({selector:"button:contains('Buy')"}).invalid_selector,true);
// Discovery supplies an executable extraction recipe, preserving field semantics.
const rows = Array.from({length:3}, (_,i) => ({localName:'li',innerText:'Item '+i,
  querySelector: s => s === 'a' || s === 'a[href]' || s.includes(',') ? {} : null}));
const parent = {id:'items',nodeType:1,children:rows};
const discoveryDocument = {querySelectorAll:() => [parent]};
result = JSON.parse(vm.runInNewContext(script('discover_lists'),{document:discoveryDocument,CSS:{escape:s=>s}}));
assert.equal(result[0].extract_example.item,'#items > li');
assert.equal(result[0].extract_example.fields.title,'a');
assert.equal(result[0].extract_example.fields.link.attr,'href');
console.log('PASS: page scripts: role/name matching, labels, visibility, duplicate refs, priority, URLs, observations, extraction recipes');
