// What covers the page while it cannot reach its worker (DECISIONS D55):
// reconnecting, open in another tab, signed out. Edits stay in the editor
// underneath. While the engine compiles in the page, a lost connection is a
// notice at the top that covers nothing (`cover: false`, D172).
export interface OverlayContent {
  title: string
  text: string
  action?: { label: string; run(): void }
}

export class Overlay {
  private element: HTMLElement | undefined

  show(content: OverlayContent, { cover = true }: { cover?: boolean } = {}): void {
    const element = this.element ?? this.create()
    element.classList.toggle('notice', !cover)
    if (cover) element.setAttribute('aria-modal', 'true')
    else element.removeAttribute('aria-modal')
    element.setAttribute('role', cover ? 'alertdialog' : 'status')
    const title = document.createElement('h2')
    title.textContent = content.title
    const text = document.createElement('p')
    text.textContent = content.text
    const children: HTMLElement[] = [title, text]
    if (content.action) {
      const { label, run } = content.action
      const button = document.createElement('button')
      button.type = 'button'
      button.className = 'primary'
      button.textContent = label
      button.addEventListener('click', run)
      children.push(button)
    }
    element.firstElementChild!.replaceChildren(...children)
    element.hidden = false
    if (cover) element.querySelector('button')?.focus()
  }

  hide(): void {
    if (this.element) this.element.hidden = true
  }

  private create(): HTMLElement {
    const element = document.createElement('div')
    element.className = 'web-overlay'
    element.setAttribute('role', 'alertdialog')
    element.setAttribute('aria-modal', 'true')
    element.append(document.createElement('div'))
    document.body.append(element)
    this.element = element
    return element
  }
}
