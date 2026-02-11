import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import Login from './pages/Login'
import Setup from './pages/Setup'
import AddDevice from './pages/AddDevice'
import Chat from './pages/Chat'
import Repos from './pages/Repos'
import { getToken } from './stores/auth'

function App() {
  return (
    <BrowserRouter>
      <div className="min-h-screen bg-gray-900 text-gray-100">
        <Routes>
          <Route path="/setup" element={<Setup />} />
          <Route path="/login" element={<Login />} />
          <Route path="/add-device" element={<AddDevice />} />
          <Route path="/repos" element={<Repos />} />
          <Route path="/chat" element={<Chat />} />
          <Route path="/" element={<IndexRedirect />} />
        </Routes>
      </div>
    </BrowserRouter>
  )
}

function IndexRedirect() {
  const token = getToken()
  if (token) return <Navigate to="/chat" replace />
  return <Navigate to="/login" replace />
}

export default App
