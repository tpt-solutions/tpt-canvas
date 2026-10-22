# Test font provenance

All fonts here are from the Noto project and are licensed under the SIL Open
Font License 1.1 (see `OFL.txt`). They are vendored only for tests.

| File | Source | Used by |
| --- | --- | --- |
| `NotoSans-Regular.ttf` | github.com/notofonts/notofonts.github.io (hinted, static) | Latin tests and goldens |
| `NotoSansArabic-Regular.ttf` | github.com/notofonts/notofonts.github.io (hinted, static) | Arabic tests and goldens |
| `NotoSansDevanagari-Regular.ttf` | github.com/notofonts/notofonts.github.io (hinted, static) | Reserved for Phase 2 fallback tests |
| `NotoEmoji-Regular.ttf` | github.com/google/fonts `ofl/notoemoji` (variable, `wght`) | Reserved for emoji fallback tests |

Static Noto Sans and Noto Sans Arabic were chosen over variable builds so
that test output does not depend on variation-axis handling.
