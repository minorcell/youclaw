import youClawLogo from '../../../../src-tauri/icons/icon.svg'

export function ChatStartPage() {
  return (
    <div className='mx-auto flex min-h-[62vh] max-w-3xl flex-col items-center justify-center px-6 text-center'>
      <img
        alt='YouClaw'
        className='h-24 w-24 rounded-[28px] shadow-sm'
        draggable={false}
        src={youClawLogo}
      />
      <h1 className='mt-6 text-2xl font-semibold tracking-tight text-foreground'>在聊天中搞定一切。</h1>
    </div>
  )
}
