---
status: accepted
---

# Every icon and mark is a Remix icon

ADR 0080 drew a field's, a select's, a checkbox's and a list's own marks as Remix icons, and left the rest as characters "for now". Those were the status marks of alerts, toasts, badges and form messages (`i`, `✓`, `!`, `×`), the mark of a wrong value (`!`), a menu radio item's `●`, a broken image's `×`, a sorted column's `▲`/`▼`, a breadcrumb's `/`, a toast's dismiss `×`, and the icons a badge, a menu item, a tab, a navigation row and a segment take. The owner asked on 2026-10-29 for the text icons to use the Remix icons too.

**Decision.** Every icon and mark ODK draws is a Remix icon from the same 4.8.0 set (ADR 0080), and `odk::ui::Icon` grows to the ones these need.

- **Status marks.** `Icon::status(role)` gives a status its icon: `Info` (`information-line`), `Success` (`checkbox-circle-line`), `Warning` (`error-warning-line`) and `Danger` (`close-circle-line`); accent and neutral have none. Alerts, toasts, marked badges and a `FormField`'s messages draw it in the colour the character had.
- **Other marks.**
  - A wrong value's mark, at a field's or a select's end and after a check box's or radio's label, is `Warning`. It is still not the clear button's `Close`.
  - A menu radio item's chosen one is `Dot`, at half the icon size.
  - A broken image shows `Image`.
  - A sorted column shows `ArrowUp` or `ArrowDown`.
  - A breadcrumb's separator is `ChevronRight` by default.
  - A toast's dismiss button is `Close`.
- **What components take.** `Badge::icon`, `MenuItem::icon`, `NavList::icon`, `Tabs::icon`, `ToggleGroup::icon` and `icon_item`, and `Breadcrumb::separator` take a `Glyph`, so an icon or a character, as buttons and fields already do. An icon in a row, a tab or a segment follows its pointer states through a group, as in a button.
- **New icons.** `ChevronUp`, `ChevronRight`, `ArrowUp`, `ArrowDown`, `More`, `Settings`, `Dot`, `Image`, and, for the gallery's demonstrations and an application's use, `FileAdd`, `FolderOpen`, `Inbox`, `Send`, `Draft`, `Spam`, `List`, `Grid`, `Columns`, `Bold`, `Italic`, `Underline`, `User`, `Team`, `BankCard`, `Notification`, `At` and `Percent`.
- **Still text.** A spinner's frames and a key cap's label stay text: a spinner is motion (an icon that turns is a change under ADR 0059), and a key cap shows the key's name. The `…` of a breadcrumb's folded middle is the label of its menu link. A password's dots are text.

`status_mark` and `FIELD_INVALID_MARK` in `odk-theme` keep the characters, for text that wants one and for the tokens' own diagnostics; no component draws them.

**Considered options.**

- **Keeping status marks as characters.** They were the most visible set left, and a circled icon reads as a status in any font.
- **Painting the radio dot**, as a `RadioGroup` paints its own. A menu draws every mark through the same icon path, and `Dot` keeps it there.
