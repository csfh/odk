-- Adds "Fetch a quote". A request goes to the application, which decides about it and answers
-- later; the answer comes to `on_network`, and `result_text` is what the status line shows.
local waiting = nil
local result = nil

ui.add("note-actions", { label = "Fetch a quote", action = "fetch_quote" })

function fetch_quote(id)
    result = nil
    waiting = network.request({ url = "https://quotes.example/random" })
    return "asking..."
end

function on_network(id, ok, payload)
    if id == waiting then
        waiting = nil
        if ok then
            result = payload
        else
            result = "failed: " .. payload
        end
    end
end

function result_text()
    return result
end
