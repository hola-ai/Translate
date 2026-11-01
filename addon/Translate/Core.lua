local addonName, ns = ...

-- One event frame, one handler table keyed by event name.
ns.handlers = {}
local frame = CreateFrame("Frame")
ns.eventFrame = frame

function ns.On(event, fn)
    ns.handlers[event] = fn
    frame:RegisterEvent(event)
end

frame:SetScript("OnEvent", function(_, event, ...)
    local fn = ns.handlers[event]
    if fn then fn(...) end
end)

local DEFAULTS = {
    showLinks = false, -- add a clickable [T] after English chat lines
    strip = true,      -- draw new English lines as a pixel strip for the companion
    trade = true,      -- translate the Trade channel too
}

function ns.Print(msg)
    DEFAULT_CHAT_FRAME:AddMessage("|cff66ccffTranslate:|r " .. msg)
end

-- Secret values (Midnight) cannot be compared or concatenated.
function ns.IsSecret(value)
    return issecretvalue ~= nil and issecretvalue(value) == true
end

ns.On("ADDON_LOADED", function(name)
    if name ~= addonName then return end
    TranslateDB = TranslateDB or {}
    for k, v in pairs(DEFAULTS) do
        if TranslateDB[k] == nil then TranslateDB[k] = v end
    end
    ns.db = TranslateDB
    frame:UnregisterEvent("ADDON_LOADED")
end)

ns.On("PLAYER_LOGIN", function()
    if ns.InitChat then ns.InitChat() end
end)

local function ShowHelp()
    ns.Print("/tr <Korean>  put its English on the clipboard; /tr zh <Korean> for Chinese")
    ns.Print("/tr  open the translate window on the latest English line")
    ns.Print("/tr links  toggle the [T] links after English lines")
    ns.Print("/tr trade  toggle translating the Trade channel")
    ns.Print("/tr test  show a test line in the overlay, without saying anything in chat")
    ns.Print("/tr strip  toggle the pixel strip the companion reads (top-left corner)")
    ns.Print("/tr strippos <x> <y>  move the strip, in screen pixels from the top-left corner")
    ns.Print("/tr probe  report which client features this addon found")
end

local commands = {
    [""] = function() ns.ShowLatest() end,
    links = function()
        ns.db.showLinks = not ns.db.showLinks
        ns.Print("[T] links " .. (ns.db.showLinks and "on." or "off."))
    end,
    trade = function()
        ns.db.trade = not ns.db.trade
        ns.Print("Trade channel translation " .. (ns.db.trade and "on." or "off."))
    end,
    strip = function()
        ns.db.strip = not ns.db.strip
        ns.SetStripShown(ns.db.strip)
        ns.Print("Pixel strip " .. (ns.db.strip and "on." or "off. The overlay gets no new lines."))
    end,
    test = function()
        ns.SendTestLine("LFM ICC 25 need MT and heals PST")
        ns.Print("Sent a test line to the strip. The companion overlay should show it in Korean shortly.")
    end,
    probe = function()
        local _, _, _, toc = GetBuildInfo()
        ns.Print("Interface " .. tostring(toc))
        ns.Print("issecretvalue: " .. tostring(issecretvalue ~= nil))
        ns.Print("Chat filter API: " .. tostring(ns.filterSource))
        ns.Print("Link click hook: " .. tostring(ns.linkSource))
        local w, h = GetPhysicalScreenSize()
        ns.Print("Physical screen: " .. tostring(w) .. "x" .. tostring(h) .. ", strip " .. (ns.db.strip and "on" or "off"))
    end,
    help = ShowHelp,
}

SLASH_TRANSLATE1 = "/tr"
SLASH_TRANSLATE2 = "/translate"
SLASH_TRANSLATE3 = "/ㅅㄱ" -- /tr typed with the Korean IME still on
SLASH_TRANSLATE4 = "/번역"
SlashCmdList.TRANSLATE = function(input)
    local text = strtrim(input or "")
    local cmd = strlower(text)
    local x, y = cmd:match("^strippos%s+(%d+)%s+(%d+)$")
    if x then
        ns.SetStripPosition(tonumber(x), tonumber(y))
        ns.Print("Pixel strip moved to " .. x .. "," .. y .. ".")
        return
    end
    local fn = commands[cmd]
    if fn then
        fn()
        return
    end
    -- Anything else is a reply to translate: "/tr 멈춰" to English, "/tr zh 멈춰" to Chinese.
    local lang, body = text:match("^(%a%a)%s+(.+)$")
    if lang ~= "en" and lang ~= "zh" then
        lang, body = "en", text
    end
    ns.SendReply(body, lang)
end
