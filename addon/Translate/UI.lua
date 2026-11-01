local addonName, ns = ...

-- The window: the original line (highlighted, ready for Ctrl+C) on top,
-- a paste box for the companion's Korean below, and the result under it.
local window, sourceBox, pasteBox, resultText, titleText
local currentId

local function MakeEditBox(parent)
    local box = CreateFrame("EditBox", nil, parent, "InputBoxTemplate")
    box:SetAutoFocus(false)
    box:SetHeight(24)
    box:SetFontObject(ChatFontNormal)
    box:SetScript("OnEscapePressed", function() window:Hide() end)
    return box
end

local function Build()
    window = CreateFrame("Frame", "TranslateWindow", UIParent, "BackdropTemplate")
    window:SetSize(460, 190)
    window:SetPoint("CENTER", 0, 120)
    window:SetFrameStrata("DIALOG")
    window:SetBackdrop({
        bgFile = "Interface\\Tooltips\\UI-Tooltip-Background",
        edgeFile = "Interface\\Tooltips\\UI-Tooltip-Border",
        tile = true, tileSize = 16, edgeSize = 16,
        insets = { left = 4, right = 4, top = 4, bottom = 4 },
    })
    window:SetBackdropColor(0.05, 0.05, 0.08, 0.92)
    window:SetMovable(true)
    window:SetClampedToScreen(true)
    window:EnableMouse(true)
    window:RegisterForDrag("LeftButton")
    window:SetScript("OnDragStart", window.StartMoving)
    window:SetScript("OnDragStop", window.StopMovingOrSizing)
    tinsert(UISpecialFrames, "TranslateWindow")

    titleText = window:CreateFontString(nil, "OVERLAY", "GameFontNormal")
    titleText:SetPoint("TOPLEFT", 14, -12)

    local close = CreateFrame("Button", nil, window, "UIPanelCloseButton")
    close:SetPoint("TOPRIGHT", -2, -2)

    local prev = CreateFrame("Button", nil, window, "UIPanelButtonTemplate")
    prev:SetSize(26, 20)
    prev:SetText("<")
    prev:SetPoint("TOPRIGHT", -64, -8)
    prev:SetScript("OnClick", function() ns.Step(-1) end)

    local nextBtn = CreateFrame("Button", nil, window, "UIPanelButtonTemplate")
    nextBtn:SetSize(26, 20)
    nextBtn:SetText(">")
    nextBtn:SetPoint("LEFT", prev, "RIGHT", 2, 0)
    nextBtn:SetScript("OnClick", function() ns.Step(1) end)

    local hint1 = window:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    hint1:SetPoint("TOPLEFT", 14, -36)
    hint1:SetText("1. Copy this line (Ctrl+C or Cmd+C)")

    sourceBox = MakeEditBox(window)
    sourceBox:SetPoint("TOPLEFT", 20, -50)
    sourceBox:SetPoint("RIGHT", -16, 0)
    -- Read-only: any typing puts the original back.
    sourceBox:SetScript("OnTextChanged", function(self, userInput)
        if userInput and currentId then
            self:SetText(ns.CopyText(currentId))
            self:HighlightText()
        end
    end)
    sourceBox:SetScript("OnEditFocusGained", function(self) self:HighlightText() end)
    sourceBox:SetScript("OnMouseUp", function(self) self:HighlightText() end)

    local hint2 = window:CreateFontString(nil, "OVERLAY", "GameFontHighlightSmall")
    hint2:SetPoint("TOPLEFT", 14, -82)
    hint2:SetText("2. When the overlay says ready, click here and paste")

    pasteBox = MakeEditBox(window)
    pasteBox:SetPoint("TOPLEFT", 20, -96)
    pasteBox:SetPoint("RIGHT", -16, 0)
    pasteBox:SetScript("OnTextChanged", function(self, userInput)
        if not userInput then return end
        local text = self:GetText()
        if text == "" then return end
        self:SetText("")
        ns.ReceiveTranslation(text, currentId)
    end)

    resultText = window:CreateFontString(nil, "OVERLAY", "GameFontHighlight")
    resultText:SetPoint("TOPLEFT", 16, -128)
    resultText:SetPoint("BOTTOMRIGHT", -16, 12)
    resultText:SetJustifyH("LEFT")
    resultText:SetJustifyV("TOP")
    resultText:SetWordWrap(true)
end

function ns.ShowMessage(id)
    local entry = ns.messages[id]
    if not entry then
        ns.Print("No English chat lines captured yet.")
        return
    end
    if not window then Build() end
    currentId = id
    titleText:SetText("Translate  |cff999999#" .. id .. "  " .. (entry.author or "") .. "|r")
    sourceBox:SetText(ns.CopyText(id))
    sourceBox:SetCursorPosition(0)
    resultText:SetText(ns.translations[id] or "|cff999999No translation yet.|r")
    window:Show()
    sourceBox:SetFocus()
    sourceBox:HighlightText()
end

function ns.Step(delta)
    if not currentId then return end
    local id = currentId + delta
    if ns.messages[id] then ns.ShowMessage(id) end
end

function ns.RefreshResult(id)
    if window and window:IsShown() and currentId == id then
        resultText:SetText(ns.translations[id] or "")
    end
end
