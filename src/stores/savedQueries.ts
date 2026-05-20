import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export interface SavedQuery {
  id: string
  name: string
  sql: string
  updatedAt: number
}

const STORAGE_KEY = 'databox_saved_queries'

function loadFromStorage(): SavedQuery[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

function persist(items: SavedQuery[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(items))
}

export const useSavedQueriesStore = defineStore('savedQueries', () => {
  const items = ref<SavedQuery[]>(loadFromStorage())

  const sorted = computed(() =>
    [...items.value].sort((a, b) => b.updatedAt - a.updatedAt),
  )

  function add(name: string, sql: string): SavedQuery {
    const item: SavedQuery = {
      id: crypto.randomUUID(),
      name,
      sql,
      updatedAt: Date.now(),
    }
    items.value.unshift(item)
    persist(items.value)
    return item
  }

  function update(id: string, updates: Partial<Pick<SavedQuery, 'name' | 'sql'>>): void {
    const idx = items.value.findIndex((q) => q.id === id)
    if (idx !== -1) {
      items.value[idx] = { ...items.value[idx], ...updates, updatedAt: Date.now() }
      persist(items.value)
    }
  }

  function remove(id: string): void {
    items.value = items.value.filter((q) => q.id !== id)
    persist(items.value)
  }

  function get(id: string): SavedQuery | undefined {
    return items.value.find((q) => q.id === id)
  }

  return {
    items,
    sorted,
    add,
    update,
    remove,
    get,
  }
})
