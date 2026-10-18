"""Small dependency-free stdio client shared by offline and opt-in live tests."""
import json
import os
import select
import subprocess
import tempfile
import time


class Client:
    def __init__(self, command, env=None):
        self.log = tempfile.TemporaryFile(mode='w+b')
        self.p = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=self.log, env=env, bufsize=0)
        self.seq = 0
        self.buffer = b''
        self.info = self.request('initialize', {
            'protocolVersion': '2024-11-05', 'capabilities': {},
            'clientInfo': {'name': 'firefox-rdp-mcp-regression', 'version': '1'},
        })
        self.send({'method': 'notifications/initialized'})

    def send(self, obj):
        self.p.stdin.write((json.dumps({'jsonrpc': '2.0', **obj}) + '\n').encode())
        self.p.stdin.flush()

    def request(self, method, params):
        self.seq += 1
        self.send({'id': self.seq, 'method': method, 'params': params})
        deadline = time.monotonic() + 30
        while True:
            if b'\n' not in self.buffer:
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not select.select([self.p.stdout], [], [], remaining)[0]:
                    raise TimeoutError(method)
                data = os.read(self.p.stdout.fileno(), 65536)
                if not data:
                    raise RuntimeError('MCP exited unexpectedly; stdout must contain JSON-RPC only')
                self.buffer += data
                continue
            line, self.buffer = self.buffer.split(b'\n', 1)
            response = json.loads(line)
            if response.get('id') == self.seq:
                return response

    def call(self, name, arguments=None, error=False):
        started = time.monotonic()
        response = self.request('tools/call', {'name': name, 'arguments': arguments or {}})
        failed = 'error' in response or response.get('result', {}).get('isError', False)
        assert failed == error, response
        elapsed = time.monotonic() - started
        print(json.dumps({'tool': name, 'seconds': round(elapsed, 3), 'expected_error': error}), flush=True)
        if error:
            return response, elapsed
        result = response['result']
        data = result.get('structuredContent')
        if data is None:
            data = json.loads(result['content'][0]['text'])
        assert len(json.dumps(data, ensure_ascii=False, separators=(',', ':')).encode()) <= 16000
        return data, elapsed

    def close(self):
        self.p.terminate()
        try:
            self.p.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.p.kill()
            self.p.wait(timeout=5)
        self.log.close()
