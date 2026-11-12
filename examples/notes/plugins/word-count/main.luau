-- Adds "Count words" to the Actions menu. The application exposes the note as `note`; the action
-- returns the text the application shows in its status line.
ui.add("note-actions", { label = "Count words", action = "count_words" })

function count_words(id)
    local note = app.get("note") or ""
    local n = 0
    for _ in string.gmatch(note, "%S+") do
        n = n + 1
    end
    return n .. " words"
end
