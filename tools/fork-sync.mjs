#!/usr/bin/env bun
import { $ } from 'bun';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, dirname, basename } from 'node:path';

const OUTDIR = '.agents/fork-sync';
const ZD = process.env.ZED_REPO || '/home/aa/repos/ide_ls/learn_ls/zed';

function parseArgs(argv) {
  const args = {};
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--list') args.list = true;
    else if (a === '--old') args.old = argv[++i];
    else if (a === '--new') args.new = argv[++i];
    else if (a === '--batch') args.batch = argv[++i];
    else if (a === '--note') args.note = argv[++i];
  }
  if (args.old && !args.new) {
    // allow positional? keep simple
  }
  if (args.list) return args;
  if (!args.old) args.old = 'bd747337';
  if (!args.new) {
    // try origin/main if available
  }
  return args;
}
async function main() {
  const args = parseArgs(process.argv);
  if (args.list) {
    // replicate list
    try {
      const dir = OUTDIR;
      const files = (await $`ls -1 ${dir}/*.md`.quiet().text()).trim().split(/\n/).filter(Boolean);
      console.log(`同步计划（${process.cwd()}/${OUTDIR}）：`);
      for (const f of files) {
        const bn = basename(f);
        if (bn === 'README.md') continue;
        const txt = readFileSync(f, 'utf8');
        const m = txt.match(/^# 同步 ([^\n]+)/m);
        const m2 = txt.match(/🟡[^|]*|🟢[^|]*|🔴[^|]*|⚪[^|]*/);
        console.log(`  ${bn.padEnd(28)} ${(m2?.[0]||'?').trim().padEnd(10)} ${m?m[1]:''}`);
      }
    } catch (e) {
      console.error(e);
    }
    return;
  }
  console.log('stub: new not fully implemented');
}
main();
