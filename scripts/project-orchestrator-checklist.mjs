#!/usr/bin/env node

import fs from 'node:fs'
import process from 'node:process'

const TRAILER_PREFIX = 'Orchestrator-Checklist-Item: '
const MAX_ITEMS = 64
const MAX_ITEM_CHARS = 600

function normalizeText(value) {
  return String(value ?? '').replace(/\s+/g, ' ').trim()
}

export function checklistItemsFromCommitMessage(message) {
  const items = []
  const seen = new Set()
  for (const line of String(message ?? '').split(/\r?\n/)) {
    if (!line.startsWith(TRAILER_PREFIX)) continue
    const raw = line.slice(TRAILER_PREFIX.length)
    const normalized = normalizeText(raw)
    if (!normalized) {
      throw new Error('Orchestrator checklist trailer must not be empty.')
    }
    if (normalized.length > MAX_ITEM_CHARS) {
      throw new Error(`Orchestrator checklist trailer exceeds ${MAX_ITEM_CHARS} characters.`)
    }
    if (!seen.has(normalized)) {
      seen.add(normalized)
      items.push(normalized)
    }
    if (items.length > MAX_ITEMS) {
      throw new Error(`Orchestrator checklist projection exceeds ${MAX_ITEMS} items.`)
    }
  }
  return items
}

export function projectChecklist({ body, commitMessage, expectedHeadSha, actualHeadSha }) {
  if (!/^[0-9a-f]{40}$/i.test(String(expectedHeadSha ?? ''))) {
    throw new Error('expectedHeadSha must be one exact 40-character commit SHA.')
  }
  if (String(actualHeadSha ?? '') !== String(expectedHeadSha)) {
    throw new Error(
      `PR head changed before checklist projection: expected ${expectedHeadSha}, found ${actualHeadSha}.`,
    )
  }

  const items = checklistItemsFromCommitMessage(commitMessage)
  if (items.length === 0) {
    return { changed: false, body: String(body ?? ''), matchedItems: [] }
  }

  const lines = String(body ?? '').split(/\n/)
  const parsed = lines.map((line, index) => {
    const match = line.match(/^(\s*-\s+\[)([ xX])(\]\s+)(.*?)(\r?)$/)
    if (!match) return null
    return {
      index,
      prefix: match[1],
      mark: match[2],
      separator: match[3],
      text: match[4],
      cr: match[5],
      normalized: normalizeText(match[4]),
    }
  })

  const matchedItems = []
  let changed = false
  for (const item of items) {
    const matches = parsed.filter((candidate) => candidate?.normalized === item)
    if (matches.length !== 1) {
      throw new Error(
        `Checklist trailer ${JSON.stringify(item)} matched ${matches.length} PR checkbox lines; exact unique match required.`,
      )
    }
    const target = matches[0]
    matchedItems.push(item)
    if (target.mark.toLowerCase() === 'x') continue
    lines[target.index] = `${target.prefix}x${target.separator}${target.text}${target.cr}`
    changed = true
  }

  return { changed, body: lines.join('\n'), matchedItems }
}

function argument(name) {
  const index = process.argv.indexOf(name)
  if (index < 0 || index + 1 >= process.argv.length) {
    throw new Error(`Missing required argument ${name}.`)
  }
  return process.argv[index + 1]
}

function main() {
  const bodyFile = argument('--body-file')
  const commitMessageFile = argument('--commit-message-file')
  const outputFile = argument('--output')
  const expectedHeadSha = argument('--expected-head-sha')
  const actualHeadSha = argument('--actual-head-sha')

  const result = projectChecklist({
    body: fs.readFileSync(bodyFile, 'utf8'),
    commitMessage: fs.readFileSync(commitMessageFile, 'utf8'),
    expectedHeadSha,
    actualHeadSha,
  })
  fs.writeFileSync(outputFile, `${JSON.stringify(result, null, 2)}\n`, 'utf8')
}

if (import.meta.url === `file://${process.argv[1]}`) {
  try {
    main()
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  }
}
