#!/usr/bin/env python3
"""Offline contract test: no browser, browser profile or external network."""
import os
import sys
from mcp_client import Client

client = Client(sys.argv[1:] or ['./result/bin/firefox-rdp-mcp'], env={
    **os.environ, 'FF_RDP_HOST': '127.0.0.1', 'FF_RDP_PORT': '9', 'FF_RDP_TIMEOUT_MS': '200',
})
try:
    assert client.info['result']['serverInfo']['name'] == 'firefox-rdp-mcp'
    tools = client.request('tools/list', {})['result']['tools']
    names = {tool['name'] for tool in tools}
    assert len(names) == len(tools) == 24, names
    assert {'firefox_status', 'firefox_navigate', 'firefox_wait', 'firefox_extract',
            'firefox_discover_lists', 'firefox_screenshot'} <= names
    for tool in tools:
        assert 'timeout_ms' in tool['inputSchema']['properties'], tool['name']
    # Invalid inputs are rejected before contacting Firefox.
    client.call('firefox_navigate', {'url': 'javascript:alert(1)'}, error=True)
    client.call('firefox_wait', {'ready': 'invalid'}, error=True)
    client.call('firefox_storage', {'area': 'invalid'}, error=True)
    client.call('not_a_tool', {}, error=True)
    # Backend unavailability is an MCP error; the server stays responsive.
    client.call('firefox_status', {'timeout_ms': 500}, error=True)
    assert client.request('tools/list', {})['result']['tools']
    assert 'error' in client.request('does/not/exist', {})
    print('PASS: initialize, tool schemas, input validation, backend failure, protocol framing')
finally:
    client.close()
