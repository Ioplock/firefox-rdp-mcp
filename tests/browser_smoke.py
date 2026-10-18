#!/usr/bin/env python3
"""Opt-in integration test against the EXISTING shared Firefox.
Run: nix shell nixpkgs#python3 --command python3 tests/browser_smoke.py ./result/bin/firefox-rdp-mcp
Creates and removes only its own local fixture tabs; never navigates existing tabs.
"""
import http.server
import os
import json
import socket
import sys
import threading
import time

class Fixture(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith('/slow'):
            time.sleep(4)
            data, kind = b'x', 'image/png'
        else:
            rows = ''.join(f'<li><a href="/item/{i}?keep=yes">Item {i}</a><span> Price {i} </span></li>' for i in range(30))
            data = (f'<html><head><title>MCP regression fixture</title></head><body><h1>Ready</h1>'
                    f'<header><button>Chrome action</button></header><main><label for="input">Search term</label><input id="input"><button id="button" onclick="this.textContent=\'Clicked\'">Click</button>'
                    f'<button class="duplicate">Duplicate</button><button class="duplicate">Duplicate</button><button hidden>Hidden action</button><ul>{rows}</ul></main><img src="/slow?{time.time_ns()}"></body></html>').encode()
            kind = 'text/html'
        self.send_response(200)
        self.send_header('Content-Type', kind)
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        try:
            self.wfile.write(data)
        except BrokenPipeError:
            pass
    def log_message(self, *args):
        pass

from mcp_client import Client

# Fixed chrome cleanup operation: remove only context IDs returned by this test.
def close_test_tabs(ids):
    # A failed navigation must never turn an original tab into a cleanup target.
    ids = [context for context in ids if f'bc:{context}' not in (originals or {})]
    if not ids:
        return
    sock = socket.create_connection((os.environ.get('FF_RDP_HOST', '127.0.0.1'), int(os.environ.get('FF_RDP_PORT', '6000'))), 5)
    sock.settimeout(5)
    def read():
        digits = b''
        while (c := sock.recv(1)) != b':':
            if not c:
                raise EOFError
            digits += c
        data = b''
        while len(data) < int(digits):
            data += sock.recv(int(digits)-len(data))
        return json.loads(data)
    def ask(value):
        data = json.dumps(value).encode()
        sock.sendall(str(len(data)).encode()+b':'+data)
        while True:
            result = read()
            if result.get('from') == value['to'] and 'type' not in result:
                assert 'error' not in result, result
                return result
    read()
    descriptor = ask({'to':'root','type':'getProcess','id':0})
    target = ask({'to':descriptor['processDescriptor']['actor'],'type':'getTarget'})
    expression = ('JSON.stringify((()=>{const ids='+json.dumps(ids)+';const wins=Services.wm.getEnumerator("navigator:browser");'
                  'while(wins.hasMoreElements()){const w=wins.getNext();for(const t of Array.from(w.gBrowser.tabs))'
                  'if(ids.includes(t.linkedBrowser.browsingContext.id))w.gBrowser.removeTab(t);}return true;})())')
    ack = ask({'to':target['process']['consoleActor'],'type':'evaluateJSAsync','text':expression,'options':{}})
    while True:
        result = read()
        if result.get('type') == 'evaluationResult' and result.get('resultID') == ack['resultID']:
            assert not result.get('hasException'), result
            break
    sock.close()

if os.environ.get('FF_RDP_HOST', '127.0.0.1') not in ('127.0.0.1', 'localhost'):
    raise SystemExit('Live fixture requires a local Firefox; FF_RDP_HOST must be loopback')

server = http.server.ThreadingHTTPServer(('127.0.0.1',0), Fixture)
threading.Thread(target=server.serve_forever, daemon=True).start()
url = f'http://127.0.0.1:{server.server_port}/'
c = Client(sys.argv[1:] or ['./result/bin/firefox-rdp-mcp'])
created = []
originals = None
try:
    baseline, _ = c.call('firefox_status')
    originals = {t['tab']:t['url'] for t in baseline['tabs']}
    data, elapsed = c.call('firefox_navigate', {'url':url,'new_tab':True})
    key = data['results']['tab']
    assert key not in originals, data
    created.append(int(key.removeprefix('bc:')))
    assert data['results']['readiness']['content_ready'], data
    assert elapsed < 3, ('interactive navigation waited for slow image',elapsed)
    status, _ = c.call('firefox_status')
    assert all(any(t['tab']==key and t['url']==value for t in status['tabs']) for key,value in originals.items())
    data, _ = c.call('firefox_extract', {'item':'li','fields':{'title':'a','url':{'select':'a','attr':'href'}},'limit':3,'offset':2})
    assert data['results']['rows'][0]['title']=='Item 2', data
    assert data['results']['next_offset']==5 and data['results']['has_more'], data
    assert '?keep=yes' in data['results']['rows'][0]['url'], data
    data, _ = c.call('firefox_discover_lists')
    recipe = next(x['extract_example'] for x in data['lists'] if x['count']==30)
    data, _ = c.call('firefox_extract', recipe)
    assert data['results']['rows'][0]['title']=='Item 0', data
    data, _ = c.call('firefox_snapshot', {'limit':2})
    assert data['elements'][0]['zone']=='content', data
    assert data['has_more'] and data['next_offset']==2, data
    data, _ = c.call('firefox_find_elements', {'role':'button','name':'Duplicate'})
    assert data['total']==2 and len(data['elements'])==2, data
    assert all(x['ref'].startswith('e') for x in data['elements'])
    data, _ = c.call('firefox_find_elements', {'role':'button','name':'Hidden action'})
    assert data['total']==0, data
    data, _ = c.call('firefox_find_elements', {'role':'textbox','name':'Search term'})
    assert data['total']==1, data
    c.call('firefox_type', {'ref':data['elements'][0]['ref'],'text':'observed', 'with_page':False,
                          'expect':{'selector':'#input','state':'text_changed'}})
    response, _ = c.call('firefox_click', {'selector':"button:contains('Click')"}, error=True)
    assert response['error']['code']==-32602, response
    data, _ = c.call('firefox_snapshot', {'format':'tree','query':'Click'})
    assert data.get('results') is not None, data
    data, _ = c.call('firefox_select_page', {'target':'MCP regression fixture'})
    assert data['selected']==key, data
    # Duplicate URLs and titles must still be independently addressable.
    duplicate, _ = c.call('firefox_navigate', {'url':url,'new_tab':True,'with_page':False})
    duplicate_key = duplicate['results']['tab']
    assert duplicate_key != key
    created.append(int(duplicate_key.removeprefix('bc:')))
    c.call('firefox_select_page', {'target':'MCP regression fixture'}, error=True)
    c.call('firefox_evaluate', {'tab':key,'expression':'location.href'})
    data, _ = c.call('firefox_close_page', {'tab':duplicate_key})
    assert data['closed'] and data['selected']!=duplicate_key, data
    assert any(t['tab']==key for t in data['tabs']), data
    response, _ = c.call('firefox_evaluate', {'tab':duplicate_key,'expression':'location.href'}, error=True)
    assert response['error']['data']['status']=='target_missing', response
    c.call('firefox_select_page', {'target':key})
    _, elapsed = c.call('firefox_wait', {'selector':'#missing','timeout_ms':500}, error=True)
    assert elapsed < 2, elapsed
    data, elapsed = c.call('firefox_navigate', {'url':url+'second','with_page':False})
    assert data['results']['tab']==key and data['results']['readiness']['state']['url'].endswith('/second'), data
    assert elapsed < 3, elapsed
    data, elapsed = c.call('firefox_navigate', {'url':url+'complete','wait':'complete','with_page':False})
    assert data['results']['readiness']['state']['ready']=='complete', data
    assert elapsed >= 3, elapsed
    c.call('firefox_type', {'selector':'#input','text':'test','with_page':False})
    data, _ = c.call('firefox_find_elements', {'role':'button','name':'Click'})
    assert data['total']==1, data
    data, _ = c.call('firefox_click', {'ref':data['elements'][0]['ref'],'with_page':False,
                                      'expect':{'selector':'#button','state':'text_changed'}})
    assert data['postcondition']['confirmed'], data
    data, _ = c.call('firefox_wait', {'condition':{'selector':'#button','state':'text_contains','text':'Clicked'}})
    assert data['confirmed'], data
    data, _ = c.call('firefox_wait', {'condition':{'selector':'#missing','state':'disappeared'}})
    assert data['confirmed'], data
    data, _ = c.call('firefox_wait', {'condition':{'selector':'#button','state':'text_contains','text':'Never appears'}})
    assert not data['confirmed'] and data['status']=='pending', data
    data, _ = c.call('firefox_evaluate', {'expression':'document.querySelector("#button").textContent'})
    assert data['results']=='Clicked', data
    _, elapsed = c.call('firefox_evaluate', {'expression':'new Promise(()=>{})','timeout_ms':500}, error=True)
    assert elapsed < 2, elapsed
    c.call('firefox_status')
    # A queued request gets its own deadline, including queue time.
    c.seq += 1
    c.send({'id':c.seq,'method':'tools/call','params':{'name':'firefox_evaluate','arguments':{'expression':'new Promise(()=>{})','timeout_ms':1500}}})
    time.sleep(0.15)
    _, elapsed = c.call('firefox_status', {'timeout_ms':300}, error=True)
    assert elapsed < 1, elapsed
    time.sleep(1.4)
    c.call('firefox_status')
    # Cancel an outstanding request; the next request must not retain its lock/process.
    c.seq += 1
    cancelled = c.seq
    c.send({'id':cancelled,'method':'tools/call','params':{'name':'firefox_evaluate','arguments':{'expression':'new Promise(()=>{})'}}})
    time.sleep(0.3)
    c.send({'method':'notifications/cancelled','params':{'requestId':cancelled,'reason':'regression test'}})
    _, elapsed = c.call('firefox_status')
    assert elapsed < 2, elapsed
    c.call('firefox_snapshot', {'tab':'bc:999999999'}, error=True)
    close_test_tabs(created)
    created.clear()
    c.call('firefox_evaluate', {'expression':'location.href'}, error=True)
    status, _ = c.call('firefox_status')
    assert {t['tab']:t['url'] for t in status['tabs']}==originals, status
    print('PASS: readiness, stable tabs, extraction, discovery, actions, deadline, closed-tab handling; original tabs preserved')
finally:
    try:
        status, _ = c.call('firefox_status')
        if originals is not None:
            created += [int(t['tab'].removeprefix('bc:')) for t in status['tabs']
                        if t['url'].startswith(url) and t['tab'] not in originals]
    except Exception:
        pass
    close_test_tabs(created)
    c.close()
    server.shutdown()
