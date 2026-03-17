const COMMENT_MARKER = '<!-- youclaw-similar-issues -->'
const ISSUE_SCAN_LIMIT = 300
const MAX_RELATED_ISSUES = 3
const RELATED_SCORE_THRESHOLD = 0.18
const DUPLICATE_CANDIDATE_THRESHOLD = 0.3

const STOP_WORDS = new Set([
  'a',
  'an',
  'and',
  'are',
  'as',
  'at',
  'be',
  'by',
  'for',
  'from',
  'how',
  'in',
  'is',
  'it',
  'of',
  'on',
  'or',
  'that',
  'the',
  'this',
  'to',
  'with',
  '问题',
  '一个',
  '一下',
  '关于',
  '出现',
  '功能',
  '支持',
  '相关',
  '需要',
])

const LABEL_DEFINITIONS = {
  'needs-triage': {
    color: 'D4C5F9',
    description: 'New issue waiting for maintainers to triage.',
  },
  bug: {
    color: 'D73A4A',
    description: 'Something is not working as expected.',
  },
  enhancement: {
    color: 'A2EEEF',
    description: 'Feature request or improvement proposal.',
  },
  question: {
    color: 'DBAB09',
    description: 'Further clarification or support is needed.',
  },
  documentation: {
    color: '0075CA',
    description: 'Documentation-related work.',
  },
  frontend: {
    color: '1D76DB',
    description: 'React, UI, routing, or styling related.',
  },
  backend: {
    color: '5319E7',
    description: 'Rust backend, storage, or service layer related.',
  },
  build: {
    color: '0E8A16',
    description: 'CI, packaging, workflow, or build pipeline related.',
  },
  provider: {
    color: 'C5DEF5',
    description: 'Provider, model, or API key related.',
  },
  'agent-runtime': {
    color: 'FBCA04',
    description: 'Agent runtime, tools, prompt, or memory related.',
  },
  'duplicate-candidate': {
    color: 'BFDADC',
    description: 'Possible duplicate based on automatic similarity detection.',
  },
}

function stripCodeBlocks(text) {
  return text.replace(/```[\s\S]*?```/g, ' ').replace(/`[^`]*`/g, ' ')
}

function normalizeWhitespace(text) {
  return text.replace(/\s+/g, ' ').trim()
}

function normalizeText(text) {
  return normalizeWhitespace(stripCodeBlocks(text).toLowerCase())
}

function tokenize(text) {
  const normalized = normalizeText(text)
    .replace(/[^\p{L}\p{N}\s_-]/gu, ' ')
    .replace(/[_-]+/g, ' ')

  return normalized.split(/\s+/).filter((token) => token.length >= 2 && !STOP_WORDS.has(token))
}

function toTokenSet(text) {
  return new Set(tokenize(text))
}

function jaccard(left, right) {
  if (left.size === 0 || right.size === 0) {
    return 0
  }

  let intersection = 0

  for (const token of left) {
    if (right.has(token)) {
      intersection += 1
    }
  }

  const union = left.size + right.size - intersection
  return union === 0 ? 0 : intersection / union
}

function overlapCount(left, right) {
  let count = 0

  for (const token of left) {
    if (right.has(token)) {
      count += 1
    }
  }

  return count
}

function getIssueText(issue) {
  return `${issue.title}\n${issue.body ?? ''}`
}

function buildIssueVector(issue) {
  const titleText = issue.title ?? ''
  const issueText = getIssueText(issue)

  return {
    title: normalizeText(titleText),
    all: normalizeText(issueText),
    titleTokens: toTokenSet(titleText),
    allTokens: toTokenSet(issueText),
  }
}

function scoreSimilarity(currentVector, candidateVector) {
  const titleScore = jaccard(currentVector.titleTokens, candidateVector.titleTokens)
  const contentScore = jaccard(currentVector.allTokens, candidateVector.allTokens)
  const sharedTitleTokens = overlapCount(currentVector.titleTokens, candidateVector.titleTokens)
  const phraseBonus =
    currentVector.title.length >= 8 &&
    candidateVector.title.length >= 8 &&
    (currentVector.title.includes(candidateVector.title) ||
      candidateVector.title.includes(currentVector.title))
      ? 0.15
      : 0
  const overlapBonus = sharedTitleTokens >= 2 ? 0.08 : 0

  return Number(
    Math.min(1, titleScore * 0.7 + contentScore * 0.3 + phraseBonus + overlapBonus).toFixed(3),
  )
}

function hasAny(text, terms) {
  return terms.some((term) => text.includes(term))
}

function collectLabels(issue) {
  const haystack = normalizeText(getIssueText(issue))
  const labels = new Set(['needs-triage'])

  if (
    hasAny(haystack, [
      'bug',
      'error',
      'fail',
      'failure',
      'broken',
      'crash',
      'panic',
      'regression',
      'unexpected',
      '异常',
      '报错',
      '错误',
      '崩溃',
      '失败',
      '不工作',
      '无法',
    ])
  ) {
    labels.add('bug')
  }

  if (
    hasAny(haystack, [
      'feature',
      'request',
      'proposal',
      'improve',
      'improvement',
      'enhancement',
      'support',
      '希望',
      '建议',
      '优化',
      '新增',
      '增加',
    ])
  ) {
    labels.add('enhancement')
  }

  if (
    issue.title.trim().endsWith('?') ||
    issue.title.trim().endsWith('？') ||
    hasAny(haystack, ['question', 'how ', 'why ', 'what ', '如何', '为什么', '是否', '怎么'])
  ) {
    labels.add('question')
  }

  if (hasAny(haystack, ['doc', 'docs', 'readme', 'documentation', '文档'])) {
    labels.add('documentation')
  }

  if (
    hasAny(haystack, [
      'react',
      'frontend',
      'ui',
      'ux',
      'page',
      'component',
      'tailwind',
      'css',
      'theme',
      'router',
      'layout',
      '前端',
      '页面',
      '组件',
      '样式',
      '主题',
    ])
  ) {
    labels.add('frontend')
  }

  if (
    hasAny(haystack, [
      'rust',
      'backend',
      'axum',
      'websocket',
      'ws ',
      'sqlite',
      'storage',
      'service',
      '后端',
      '服务',
      '数据库',
      '存储',
    ])
  ) {
    labels.add('backend')
  }

  if (
    hasAny(haystack, [
      'ci',
      'workflow',
      'github action',
      'github workflow',
      'build',
      'compile',
      'package',
      'release',
      'tauri build',
      '构建',
      '编译',
      '打包',
      '发布',
      '工作流',
    ])
  ) {
    labels.add('build')
  }

  if (
    hasAny(haystack, [
      'provider',
      'model',
      'openai',
      'anthropic',
      'api key',
      '供应商',
      '模型',
      '密钥',
    ])
  ) {
    labels.add('provider')
  }

  if (
    hasAny(haystack, [
      'agent',
      'runtime',
      'tool',
      'tool call',
      'memory',
      'prompt',
      'session',
      'summarizer',
      'agent runtime',
      '工具',
      '记忆',
      '提示词',
      '会话',
    ])
  ) {
    labels.add('agent-runtime')
  }

  return [...labels]
}

async function ensureLabels(github, repo, labels) {
  for (const label of labels) {
    const definition = LABEL_DEFINITIONS[label]

    if (!definition) {
      continue
    }

    try {
      await github.rest.issues.createLabel({
        owner: repo.owner,
        repo: repo.repo,
        name: label,
        color: definition.color,
        description: definition.description,
      })
    } catch (error) {
      if (error.status !== 422) {
        throw error
      }
    }
  }
}

async function addMissingLabels(github, repo, issueNumber, labels) {
  if (labels.length === 0) {
    return
  }

  await github.rest.issues.addLabels({
    owner: repo.owner,
    repo: repo.repo,
    issue_number: issueNumber,
    labels,
  })
}

async function loadCandidateIssues(github, repo, currentIssueNumber) {
  const issues = []
  let page = 1

  while (issues.length < ISSUE_SCAN_LIMIT) {
    const response = await github.rest.issues.listForRepo({
      owner: repo.owner,
      repo: repo.repo,
      state: 'all',
      sort: 'updated',
      direction: 'desc',
      per_page: 100,
      page,
    })

    const pageIssues = response.data.filter(
      (issue) => !issue.pull_request && issue.number !== currentIssueNumber,
    )

    issues.push(...pageIssues)

    if (response.data.length < 100) {
      break
    }

    page += 1
  }

  return issues.slice(0, ISSUE_SCAN_LIMIT)
}

function findRelatedIssues(issue, candidates) {
  const currentVector = buildIssueVector(issue)

  return candidates
    .map((candidate) => {
      const score = scoreSimilarity(currentVector, buildIssueVector(candidate))

      return { candidate, score }
    })
    .filter(({ score }) => score >= RELATED_SCORE_THRESHOLD)
    .sort((left, right) => right.score - left.score)
    .slice(0, MAX_RELATED_ISSUES)
}

function buildSimilarityComment(relatedIssues) {
  const lines = [
    COMMENT_MARKER,
    'Potentially related issues found by the issue triage workflow:',
    '',
  ]

  for (const { candidate, score } of relatedIssues) {
    lines.push(
      `- [#${candidate.number} ${candidate.title}](${candidate.html_url}) (${candidate.state}, similarity ${score})`,
    )
  }

  lines.push('')
  lines.push('If one of these already describes the same problem, consider continuing there.')

  return lines.join('\n')
}

async function syncSimilarityComment(github, repo, issueNumber, relatedIssues) {
  const existingComments = await github.paginate(github.rest.issues.listComments, {
    owner: repo.owner,
    repo: repo.repo,
    issue_number: issueNumber,
    per_page: 100,
  })

  const existingComment = existingComments.find((comment) => comment.body?.includes(COMMENT_MARKER))

  if (relatedIssues.length === 0) {
    if (!existingComment) {
      return
    }

    await github.rest.issues.deleteComment({
      owner: repo.owner,
      repo: repo.repo,
      comment_id: existingComment.id,
    })

    return
  }

  const body = buildSimilarityComment(relatedIssues)

  if (existingComment) {
    await github.rest.issues.updateComment({
      owner: repo.owner,
      repo: repo.repo,
      comment_id: existingComment.id,
      body,
    })

    return
  }

  await github.rest.issues.createComment({
    owner: repo.owner,
    repo: repo.repo,
    issue_number: issueNumber,
    body,
  })
}

module.exports.run = async function run({ github, context, core }) {
  const issue = context.payload.issue

  if (!issue || issue.pull_request) {
    core.info('No issue payload to process.')
    return
  }

  const repo = context.repo
  const existingLabels = new Set((issue.labels ?? []).map((label) => label.name))
  const desiredLabels = collectLabels(issue)
  const labelsToAdd = desiredLabels.filter((label) => !existingLabels.has(label))

  await ensureLabels(github, repo, desiredLabels)
  await addMissingLabels(github, repo, issue.number, labelsToAdd)

  const candidates = await loadCandidateIssues(github, repo, issue.number)
  const relatedIssues = findRelatedIssues(issue, candidates)

  core.info(
    `Scanned ${candidates.length} issues, found ${relatedIssues.length} related candidates.`,
  )

  if (relatedIssues.length > 0 && relatedIssues[0].score >= DUPLICATE_CANDIDATE_THRESHOLD) {
    await ensureLabels(github, repo, ['duplicate-candidate'])

    if (!existingLabels.has('duplicate-candidate')) {
      await addMissingLabels(github, repo, issue.number, ['duplicate-candidate'])
    }
  }

  await syncSimilarityComment(github, repo, issue.number, relatedIssues)
}
