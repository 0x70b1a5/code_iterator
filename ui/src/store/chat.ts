import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'
import { ChatMessage } from '../types/Chat'

export interface ChatStore {
  get: () => ChatStore
  set: (partial: ChatStore | Partial<ChatStore>) => void
  chatHistory: ChatMessage[]
  addMessage: (message: ChatMessage) => void
}

const useChatStore = create<ChatStore>()(
  persist(
    (set, get) => ({
      get,
      set,
      chatHistory: [
        {
          author: 'Admin',
          content: 'Welcome to the chat.',
        },
        {
          author: 'Admin',
          content: 'Ask the AI to write some Python for you.',
        },
        {
          author: 'Admin',
          content: 'You can make adjustments to the generated code on the left side.'
        },
        {
          author: 'Admin',
          content: 'Click "Run" when you are ready to see the results.'
        },
      ],
      addMessage: (message) => {
        if (!message.author || !message.content) return
        set((state) => ({
          chatHistory: [...state.chatHistory, message],
        }))
      },
    }),
    {
      name: 'chat', // unique name
      storage: createJSONStorage(() => sessionStorage), // (optional) by default, 'localStorage' is used
    }
  )
)

export default useChatStore
