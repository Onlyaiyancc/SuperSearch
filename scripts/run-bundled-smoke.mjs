import { spawn } from 'node:child_process';
import { describeLaunch } from './desktop.mjs';

// 复用项目工具链，只替换诊断参数；测试仅索引隔离目录。
const invocation = describeLaunch('smoke');
invocation.args[invocation.args.length - 1] = '--smoke-test-bundled';
const child = spawn(invocation.command, invocation.args, invocation.options);
child.on('error', error => { console.error(error.message); process.exitCode = 1; });
child.on('exit', code => { process.exitCode = code ?? 1; });
