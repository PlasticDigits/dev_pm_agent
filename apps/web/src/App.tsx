import { useEffect } from 'react'
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { AuthProvider, useAuth } from './contexts/AuthContext'
import Login from './pages/Login'
import Setup from './pages/Setup'
import AddDevice from './pages/AddDevice'
import Chat from './pages/Chat'
import ChatDetail from './pages/ChatDetail'
import ChatDocs from './pages/ChatDocs'
import { initializeTheme } from './theme'

function App() {
  useEffect(() => {
    initializeTheme()
  }, [])

  return (
    <AuthProvider>
      <BrowserRouter>
        <div className="app-shell">
          <Routes>
            <Route path="/setup" element={<Setup />} />
            <Route path="/login" element={<Login />} />
            <Route path="/add-device" element={<AddDevice />} />
            <Route path="/chat" element={<Chat />} />
            <Route path="/chat/:chatId" element={<ChatDetail />} />
            <Route path="/chat/:chatId/docs" element={<ChatDocs />} />
            <Route path="/" element={<IndexRedirect />} />
          </Routes>
        </div>
      </BrowserRouter>
    </AuthProvider>
  )
}

function IndexRedirect() {
  const { token } = useAuth()
  if (token) return <Navigate to="/chat" replace />
  return <Navigate to="/login" replace />
}

export default App
