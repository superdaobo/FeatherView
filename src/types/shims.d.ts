/**
 * 第三方库缺失类型声明补齐。
 */

// markdown-it-task-lists 无官方类型
declare module 'markdown-it-task-lists' {
  import type MarkdownIt from 'markdown-it'

  interface TaskListsOptions {
    enabled?: boolean
    label?: boolean
    labelAfter?: boolean
  }

  function taskLists(md: MarkdownIt, options?: TaskListsOptions): void
  export = taskLists
}

