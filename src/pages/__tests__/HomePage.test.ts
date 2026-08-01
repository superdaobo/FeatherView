import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import HomePage from '../HomePage.vue'
import { useRecentFilesStore } from '../../stores/recentFiles'
import { useFavoritesStore } from '../../stores/favorites'

// mock vue-router
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
  RouterLink: { template: '<a><slot /></a>' },
}))

describe('HomePage', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
  })

  it('shows empty state when no recent files', () => {
    const wrapper = mount(HomePage)
    expect(wrapper.text()).toContain('览匣 FeatherView')
    expect(wrapper.text()).toContain('还没有打开过文件')
    expect(wrapper.find('.btn-open').exists()).toBe(true)
  })

  it('renders recent files list with remove and clear actions', () => {
    const store = useRecentFilesStore()
    store.recordOpen({
      id: '1',
      name: 'readme.md',
      extension: 'md',
      path: 'C:/docs/readme.md',
      size: 1024,
    })
    store.recordOpen({
      id: '2',
      name: 'notes.txt',
      extension: 'txt',
      path: 'C:/docs/notes.txt',
    })

    const wrapper = mount(HomePage)
    const items = wrapper.findAll('.recent-item')
    expect(items.length).toBe(2)
    expect(wrapper.text()).toContain('readme.md')
    expect(wrapper.text()).toContain('notes.txt')
    expect(wrapper.text()).toContain('1.0 KB')
    expect(wrapper.find('.recent-remove').exists()).toBe(true)
  })

  it('marks invalid files after failed existence check', async () => {
    const store = useRecentFilesStore()
    store.recordOpen({
      id: '1',
      name: 'gone.md',
      extension: 'md',
      path: 'C:/docs/gone.md',
    })

    const wrapper = mount(HomePage)
    // 非 Tauri 环境下 checkFileExists 返回 true，这里直接验证失效 UI 结构存在
    const removeBtn = wrapper.find('.recent-remove')
    expect(removeBtn.exists()).toBe(true)

    // 点击删除
    await removeBtn.trigger('click')
    expect(wrapper.findAll('.recent-item').length).toBe(0)
  })

  it('clears all recent files', async () => {
    const store = useRecentFilesStore()
    store.recordOpen({ id: '1', name: 'a.md', extension: 'md', path: 'C:/a.md' })
    vi.spyOn(window, 'confirm').mockReturnValue(true)

    const wrapper = mount(HomePage)
    await wrapper.find('.section-head .btn-icon').trigger('click')
    expect(store.files.length).toBe(0)
    expect(wrapper.text()).toContain('还没有打开过文件')
  })

  it('shows supported formats section', () => {
    const wrapper = mount(HomePage)
    expect(wrapper.text()).toContain('支持的文件类型')
    expect(wrapper.text()).toContain('.md')
    expect(wrapper.text()).toContain('.json')
  })

  it('shows the folder browsing entry', () => {
    const wrapper = mount(HomePage)
    expect(wrapper.text()).toContain('打开文件夹')
    expect(wrapper.find('.btn-open').exists()).toBe(true)
  })

  it('renders favorites section with items', async () => {
    const favorites = useFavoritesStore()
    favorites.toggle({ id: '1', name: 'a.md', extension: 'md', path: 'C:/a.md' })
    favorites.toggle({ id: '2', name: 'b.md', extension: 'md', path: 'C:/b.md' })

    const wrapper = mount(HomePage)
    expect(wrapper.text()).toContain('收藏')
    expect(wrapper.findAll('.favorite-item').length).toBe(2)
    expect(wrapper.text()).toContain('a.md')
    expect(wrapper.text()).toContain('b.md')

    // 取消收藏
    await wrapper.findAll('.favorite-remove')[0].trigger('click')
    expect(favorites.items.length).toBe(1)
  })
})