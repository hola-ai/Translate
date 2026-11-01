local addonName, ns = ...

-- The pixel strip: the only live way out of the game. New English lines are
-- drawn as a small grid of colored blocks that the companion app reads off
-- the screen. Each block carries 6 bits: 4 brightness levels per channel,
-- far enough apart that small color shifts still decode correctly.
--
-- Frame layout, row by row from the top-left block:
--   3 sync blocks (magenta, cyan, yellow), then the payload bytes
--   [seq] [length hi] [length lo] [text...] [fletcher16 hi] [fletcher16 lo]
--   packed 3 bytes -> 4 blocks, padded with black.
local COLS, ROWS = 64, 8
local BLOCK = 4 -- physical screen pixels per block side
local SYNC = { 51, 15, 60 } -- (3,0,3) magenta, (0,3,3) cyan, (3,3,0) yellow
local CAPACITY = math.floor((COLS * ROWS - #SYNC) / 4) * 3
local MAX_TEXT = CAPACITY - 5
local SHOW_TIME = 0.25 -- seconds each frame stays up; the app polls faster
local MAX_QUEUE = 50
-- The macOS menu bar stays on top of the game and would hide the strip at y = 0.
local DEFAULT_Y = (IsMacClient and IsMacClient()) and 48 or 0

ns.STRIP = { cols = COLS, rows = ROWS, block = BLOCK, maxText = MAX_TEXT }

local function Fletcher16(bytes)
    local a, b = 0, 0
    for i = 1, #bytes do
        a = (a + bytes[i]) % 255
        b = (b + a) % 255
    end
    return b * 256 + a
end

-- Cuts at a UTF-8 character boundary so a long line stays valid text.
local function Truncate(text, max)
    if #text <= max then return text end
    local cut = max
    while cut > 0 do
        local c = text:byte(cut + 1)
        if c < 128 or c >= 192 then break end
        cut = cut - 1
    end
    return text:sub(1, cut)
end

-- Returns COLS * ROWS symbols (0..63). Pure Lua, so it can be tested outside WoW.
function ns.EncodeFrame(seq, text)
    text = Truncate(text, MAX_TEXT)
    local bytes = { seq % 256, math.floor(#text / 256), #text % 256 }
    for i = 1, #text do bytes[#bytes + 1] = text:byte(i) end
    local sum = Fletcher16(bytes)
    bytes[#bytes + 1] = math.floor(sum / 256)
    bytes[#bytes + 1] = sum % 256

    local symbols = {}
    for i = 1, #SYNC do symbols[i] = SYNC[i] end
    for i = 1, #bytes, 3 do
        local v = (bytes[i] or 0) * 65536 + (bytes[i + 1] or 0) * 256 + (bytes[i + 2] or 0)
        symbols[#symbols + 1] = math.floor(v / 262144) % 64
        symbols[#symbols + 1] = math.floor(v / 4096) % 64
        symbols[#symbols + 1] = math.floor(v / 64) % 64
        symbols[#symbols + 1] = v % 64
    end
    for i = #symbols + 1, COLS * ROWS do symbols[i] = 0 end
    return symbols
end

local frame, textures
local queue = {}
local seq = 0
local ticking = false

-- One block = one physical pixel grid cell, whatever the UI scale.
function ns.PlaceStrip()
    if not frame then return end
    local _, height = GetPhysicalScreenSize()
    frame:SetScale(768 / height)
    frame:ClearAllPoints()
    frame:SetPoint("TOPLEFT", UIParent, "TOPLEFT", ns.db.stripX or 0, -(ns.db.stripY or DEFAULT_Y))
end

local function Build()
    frame = CreateFrame("Frame", "TranslateStrip", UIParent)
    frame:SetIgnoreParentScale(true)
    frame:SetFrameStrata("TOOLTIP")
    frame:SetSize(COLS * BLOCK, ROWS * BLOCK)
    textures = {}
    for i = 0, COLS * ROWS - 1 do
        local t = frame:CreateTexture(nil, "OVERLAY")
        t:SetSize(BLOCK, BLOCK)
        t:SetPoint("TOPLEFT", (i % COLS) * BLOCK, -math.floor(i / COLS) * BLOCK)
        t:SetColorTexture(0, 0, 0, 1)
        textures[i + 1] = t
    end
    ns.PlaceStrip()
end

local function Paint(symbols)
    for i, s in ipairs(symbols) do
        local r, g, b = math.floor(s / 16) % 4, math.floor(s / 4) % 4, s % 4
        textures[i]:SetColorTexture(r / 3, g / 3, b / 3, 1)
    end
end

local function Advance()
    if #queue == 0 then
        ticking = false
        return
    end
    seq = (seq + 1) % 256
    Paint(ns.EncodeFrame(seq, table.remove(queue, 1)))
    ticking = true
    C_Timer.After(SHOW_TIME, Advance)
end

function ns.SendToStrip(text)
    if not ns.db.strip then return end
    if not frame then Build() end
    if #queue >= MAX_QUEUE then table.remove(queue, 1) end
    queue[#queue + 1] = text
    if not ticking then Advance() end
end

function ns.SetStripShown(shown)
    if shown then
        if not frame then Build() end
        frame:Show()
    elseif frame then
        frame:Hide()
    end
end

-- Moves the strip, in physical pixels from the top-left corner of the screen.
function ns.SetStripPosition(x, y)
    ns.db.stripX, ns.db.stripY = x, y
    if not frame then Build() end
    ns.PlaceStrip()
end

ns.On("DISPLAY_SIZE_CHANGED", function() ns.PlaceStrip() end)
ns.On("UI_SCALE_CHANGED", function() ns.PlaceStrip() end)
