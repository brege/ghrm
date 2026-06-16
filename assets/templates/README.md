# templates

The template tree owns document structure for server rendered pages and fragments. Askama formatting should favor readable source with whitespace control, not collapsed one line markup.

## Layout

| Path | Purpose |
| --- | --- |
| base.html | full document shell and shared page chrome |
| page.html | rendered file page shell |
| explorer.html | explorer table and README layout |
| gist.html | gist editor page |
| gist_stash.html | gist stash listing |
| fragments/ | partial responses and shared template fragments |
| macros/ | shared Askama macros grouped by feature |

## Formatting

- use 2 space indentation for HTML and template bodies
- use `{%-` and `-%}` to control whitespace instead of collapsing logic onto one line
- keep template tags indented to the HTML they produce
- keep conditional attributes on the element they belong to
- stack opening tag attributes vertically when an element has 3 or more attributes, or any conditional attribute
- keep loop and conditional bodies indented one level from the enclosing tag

## Macros

- extract a macro when the same structural HTML pattern appears 3 or more times
- group related macros in one file under `macros/`, not one file per macro
- keep guard conditions inside the macro when callers should be able to invoke it unconditionally
- prefer macros for repeated server rendered structure, not browser code

## Boundaries

- preserve existing class names, data attributes, and DOM structure unless the change is intentionally structural
- keep Rust view models and Askama templates aligned, do not shift data shaping into JavaScript to avoid template work
- keep Lit islands and runtime modules focused on behavior after render
- do not hand edit generated runtime files under `assets/js/`
