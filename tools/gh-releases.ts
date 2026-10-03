#!/usr/bin/env bun
// 查 GitHub 仓库的 release 列表 + 每个 tag 对应的 commit sha（单次 GraphQL 请求）。
//
// 用法：gh-releases.ts <owner/repo> [--limit N] [--offset N] [--latest] [--json]
//   --limit N    最多输出几条（默认 15）
//   --offset N   跳过前 N 条（默认 0）
//   --latest     只输出最新稳定版那一行（客户端筛选，会多取 LATEST_SCAN 条）
//   --json       输出原始 JSON 数组
//
// 例：
//   bun tools/gh-releases.ts zed-industries/zed --limit 15
//   bun tools/gh-releases.ts zed-industries/zed --offset 15 --limit 5
//   bun tools/gh-releases.ts zed-industries/zed --latest

const USAGE = `gh-releases.ts <owner/repo> [--limit N] [--offset N] [--latest] [--json]

  --limit N    最多输出几条（默认 15）
  --offset N   跳过前 N 条（默认 0）
  --latest     只输出最新稳定版那一行（客户端筛选，会多取 20 条）
  --json       输出原始 JSON 数组`;

const QUERY = `query($owner: String!, $name: String!, $count: Int!) {
  repository(owner: $owner, name: $name) {
    releases(first: $count, orderBy: {field: CREATED_AT, direction: DESC}) {
      nodes {
        tagName
        publishedAt
        isPrerelease
        isDraft
        isLatest
        tagCommit { oid }
      }
    }
  }
}`;

interface ReleaseNode {
  tagName: string;
  publishedAt: string | null;
  isPrerelease: boolean;
  isDraft: boolean;
  isLatest: boolean;
  tagCommit: { oid: string } | null;
}

interface Options {
  repo: string;
  limit: number;
  offset: number;
  latestOnly: boolean;
  asJson: boolean;
}

// --latest 靠客户端过滤出最新稳定版，得多取一些才能越过前面的 pre-release。
const LATEST_SCAN = 20;

class UsageError extends Error {}

function fail(message: string, code = 2): never {
  console.error(`gh-releases: ${message}`);
  process.exit(code);
}

function parseArgs(argv: string[]): Options {
  const options: Options = {
    repo: "",
    limit: 15,
    offset: 0,
    latestOnly: false,
    asJson: false,
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]!;

    switch (arg) {
      case "--limit":
      case "--offset": {
        const raw = argv[++i];
        if (raw === undefined || !/^\d+$/.test(raw)) {
          throw new UsageError(`${arg} 要是非负整数`);
        }
        if (arg === "--limit") options.limit = Number(raw);
        else options.offset = Number(raw);
        break;
      }
      case "--latest":
        options.latestOnly = true;
        break;
      case "--json":
        options.asJson = true;
        break;
      case "-h":
      case "--help":
        console.log(USAGE);
        process.exit(0);
        break;
      default:
        if (arg.startsWith("-")) throw new UsageError(`未知参数：${arg}`);
        if (options.repo) {
          throw new UsageError(`只能指定一个 repo（已有 ${options.repo}，又来了 ${arg}）`);
        }
        options.repo = arg;
    }
  }

  if (!options.repo) {
    throw new UsageError(`缺少 repo，例：gh-releases.ts zed-industries/zed`);
  }
  const segments = options.repo.split("/");
  if (segments.length !== 2 || !segments[0] || !segments[1]) {
    throw new UsageError(`repo 要写成 owner/repo，收到：${options.repo}`);
  }
  return options;
}

function fetchReleases(repo: string, count: number): ReleaseNode[] {
  const [owner, name] = repo.split("/") as [string, string];
  const proc = Bun.spawnSync([
    "gh",
    "api",
    "graphql",
    "-F",
    `owner=${owner}`,
    "-F",
    `name=${name}`,
    "-F",
    `count=${count}`,
    "-f",
    `query=${QUERY}`,
  ]);

  const stderr = proc.stderr.toString().trim();
  if (proc.exitCode !== 0) {
    fail(stderr || `gh 退出码 ${proc.exitCode}`, 1);
  }

  const payload = JSON.parse(proc.stdout.toString()) as {
    data?: { repository: { releases: { nodes: ReleaseNode[] } | null } | null } | null;
    errors?: { message: string }[];
  };

  const apiError = payload.errors?.[0];
  if (apiError) fail(apiError.message, 1);
  if (!payload.data?.repository) fail(`仓库不存在或无权访问：${repo}`, 1);

  return payload.data.repository.releases?.nodes ?? [];
}

// isLatest 只有部分 release 会被 GitHub 标记，最新稳定版自己按 isDraft/isPrerelease 判定。
function selectRows(nodes: ReleaseNode[], options: Options): ReleaseNode[] {
  if (options.latestOnly) {
    const latest = nodes.find((node) => !node.isDraft && !node.isPrerelease);
    return latest ? [latest] : [];
  }
  return nodes.slice(options.offset, options.offset + options.limit);
}

function toRow(node: ReleaseNode): string[] {
  return [
    node.tagName,
    node.tagCommit?.oid ?? "-",
    (node.publishedAt ?? "-").slice(0, 10),
    node.isPrerelease ? "pre" : "stable",
    node.isDraft ? "draft" : "",
    node.isLatest ? "latest" : "",
  ];
}

function main(): void {
  let options: Options;
  try {
    options = parseArgs(process.argv.slice(2));
  } catch (error) {
    if (error instanceof UsageError) {
      console.error(`gh-releases: ${error.message}\n`);
      console.error(USAGE);
      process.exit(2);
    }
    throw error;
  }

  // GraphQL 只有游标分页（after），没有 offset，所以一次取 offset+limit 条再本地切片。
  // --latest 是客户端筛选，多取一些，否则 limit=1 时可能只拿到 pre-release 而筛不出稳定版。
  const count = options.latestOnly
    ? Math.max(options.offset + options.limit, LATEST_SCAN)
    : options.offset + options.limit;
  const nodes = fetchReleases(options.repo, count);
  const rows = selectRows(nodes, options);

  if (options.asJson) {
    console.log(JSON.stringify(rows, null, 2));
    return;
  }
  for (const node of rows) {
    console.log(toRow(node).join("\t"));
  }
}

main();
