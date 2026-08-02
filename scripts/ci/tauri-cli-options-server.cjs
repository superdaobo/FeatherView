// 零依赖 tauri CLI options server（WebSocket JSON-RPC）
// 供裸 xcodebuild 的 tauri Build Rust Script 获取构建配置。
// 协议：HTTP Upgrade + RFC6455 文本帧（仅需响应 jsonrpsee 的 options 请求）。
'use strict';
const http = require('http');
const crypto = require('crypto');

const WS_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

const options = {
  dev: false,
  features: ['custom-protocol'],
  args: ['--lib', '--release', '--features', 'custom-protocol'],
  noise_level: 'Polite',
  vars: {},
  config: [],
  target_device: null,
};

const port = parseInt(process.env.TAURI_CLI_OPTIONS_PORT || '0', 10);

const server = http.createServer((req, res) => {
  const key = req.headers['sec-websocket-key'];
  if (!key) {
    res.writeHead(400);
    res.end();
    return;
  }
  const accept = crypto
    .createHash('sha1')
    .update(key + WS_GUID)
    .digest('base64');
  res.writeHead(101, {
    Upgrade: 'websocket',
    Connection: 'Upgrade',
    'Sec-WebSocket-Accept': accept,
  });
  res.socket.setNoDelay(true);
  handleSocket(res.socket);
});

function handleSocket(sock) {
  let buf = Buffer.alloc(0);
  sock.on('data', (chunk) => {
    buf = Buffer.concat([buf, chunk]);
    for (;;) {
      if (buf.length < 2) return;
      const fin = (buf[0] & 0x80) !== 0;
      const opcode = buf[0] & 0x0f;
      const masked = (buf[1] & 0x80) !== 0;
      let len = buf[1] & 0x7f;
      let off = 2;
      if (len === 126) {
        if (buf.length < 4) return;
        len = buf.readUInt16BE(2);
        off = 4;
      } else if (len === 127) {
        if (buf.length < 10) return;
        len = Number(buf.readBigUInt64BE(2));
        off = 10;
      }
      if (!masked || buf.length < off + 4 + len) return;
      const mask = buf.subarray(off, off + 4);
      off += 4;
      const payload = Buffer.alloc(len);
      for (let i = 0; i < len; i++) payload[i] = buf[off + i] ^ mask[i & 3];
      buf = buf.subarray(off + len);
      if (opcode === 0x1 && fin) {
        handleMessage(payload.toString('utf8'), sock);
      }
      if (opcode === 0x8) {
        sock.end();
        return;
      }
    }
  });
  sock.on('error', () => {});
}

function handleMessage(text, sock) {
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    return;
  }
  if (msg && msg.method === 'options') {
    const reply = JSON.stringify({ jsonrpc: '2.0', id: msg.id ?? 1, result: options });
    sendTextFrame(sock, reply);
  }
}

function sendTextFrame(sock, text) {
  const payload = Buffer.from(text, 'utf8');
  let header;
  if (payload.length < 126) {
    header = Buffer.from([0x81, payload.length]);
  } else if (payload.length < 65536) {
    header = Buffer.alloc(4);
    header[0] = 0x81;
    header[1] = 126;
    header.writeUInt16BE(payload.length, 2);
  } else {
    header = Buffer.alloc(10);
    header[0] = 0x81;
    header[1] = 127;
    header.writeBigUInt64BE(BigInt(payload.length), 2);
  }
  sock.write(Buffer.concat([header, payload]));
}

server.on('error', (e) => {
  console.error('server error:', e.message);
  process.exit(1);
});

server.listen(port, '127.0.0.1', () => {
  const addr = server.address();
  console.log('CLI options WS server ' + addr.address + ':' + addr.port);
});

process.stdin.resume();
