# Language Highlighting

These fixtures exercise code-block cases that should stay plain text unless a
language is explicitly declared.

## Front Matter As Plain Text

```
---
yourName: "Jane Doe"
oshea_plugin: cover-letter
---

**{{ yourName }}**

As a creator of cover letters with professional formatting...
```

## Bash Commands

```bash
git clone https://github.com/brege/oshea.git
cd oshea
npm install -g
```

## Tree Output As Plain Text

```
my-plugins/academic-letter/
├── .contract                 schema and in-situ testing
├── default.yaml              plugin config, --help text, metadata
├── style.css                 custom CSS properties
├── example.md                self-activating example
├── index.js                  handler
└── README.md                 plugin description
```

## Bash

```bash
git clone https://github.com/brege/oshea.git
cd oshea
echo "ready"
```

## JavaScript

```javascript
export function greet(name) {
  const target = name ?? 'world';
  return `hello, ${target}`;
}
```

## TypeScript

```typescript
type User = { id: string; active: boolean };

export function isActive(user: User): boolean {
  return user.active;
}
```

## Python

```python
def greet(name: str | None = None) -> str:
    target = name or "world"
    return f"hello, {target}"
```

## Rust

```rust
fn greet(name: Option<&str>) -> String {
    format!("hello, {}", name.unwrap_or("world"))
}
```

## React JSX

```jsx
export function Greeting({ name }) {
  return <p className="greeting">hello, {name ?? 'world'}</p>;
}
```

## TSX

```tsx
type GreetingProps = { name?: string };

export function Greeting({ name = 'world' }: GreetingProps) {
  return <p className="greeting">hello, {name}</p>;
}
```

## JSON

```json
{
  "name": "ghrm",
  "watch": true,
  "ports": [1313, 18187]
}
```

## YAML

```yaml
name: ghrm
theme: dark
features:
  toc: true
  live_reload: true
```

## HTML

```html
<section class="card">
  <h2>ghrm</h2>
  <p>Markdown preview server</p>
</section>
```

## CSS

```css
.card {
  border: 1px solid var(--borderColor-default);
  border-radius: 6px;
}
```

## SQL

```sql
select id, title
from docs
where published = true
order by updated_at desc;
```

## TOML

```toml
[package]
name = "ghrm"
version = "0.1.0"
edition = "2024"
```

## Diff

```diff
- old title
+ new title
```
