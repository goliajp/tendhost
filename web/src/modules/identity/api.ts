import { api } from '@/kit/api'
import type { Role } from '@/kit/session'

export interface User {
  id: string
  name: string
  role: Role
  created_at: string
}

export interface Token {
  id: string
  name: string
  role: Role
  user_id: string
  created_at: string
}

/** a token at the one moment its secret is known */
export interface NewToken extends Token {
  secret: string
}

export const identityApi = {
  setupNeeded: () => api.probe<{ needed: boolean }>('/setup').then((state) => state.needed),
  completeSetup: (setup_token: string, name: string, password: string) =>
    api.post<User>('/setup', { setup_token, name, password }, true),
  currentUser: () => api.probe<User>('/session'),
  signIn: (name: string, password: string) => api.post<User>('/session', { name, password }, true),
  signOut: () => api.delete('/session'),
  users: () => api.get<User[]>('/users'),
  createUser: (name: string, password: string, role: Role) => api.post<User>('/users', { name, password, role }),
  removeUser: (id: string) => api.delete(`/users/${encodeURIComponent(id)}`),
  tokens: () => api.get<Token[]>('/tokens'),
  createToken: (name: string, role: Role) => api.post<NewToken>('/tokens', { name, role }),
  revokeToken: (id: string) => api.delete(`/tokens/${encodeURIComponent(id)}`),
}
