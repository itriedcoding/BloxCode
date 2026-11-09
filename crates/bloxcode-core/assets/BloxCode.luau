--!nocheck
-- BloxCode Studio Bridge __BLOXCODE_VERSION__
-- Pulls AI-generated Luau from the BloxCode desktop app into this place.
-- Requires Game Settings > Security > Allow HTTP Requests to be enabled.

local HttpService = game:GetService("HttpService")
local ChangeHistoryService = game:GetService("ChangeHistoryService")
local ReplicatedStorage = game:GetService("ReplicatedStorage")
local ServerScriptService = game:GetService("ServerScriptService")
local StarterPlayer = game:GetService("StarterPlayer")

local DEFAULT_PORT = __BLOXCODE_PORT__
local DEFAULT_TOKEN = "__BLOXCODE_TOKEN__"
local POLL_SECONDS = 4
local SCRIPT_CLASSES = { Script = true, LocalScript = true, ModuleScript = true }

local theme = {
	background = Color3.fromRGB(10, 10, 10),
	panel = Color3.fromRGB(18, 18, 18),
	line = Color3.fromRGB(38, 38, 38),
	text = Color3.fromRGB(242, 242, 242),
	muted = Color3.fromRGB(140, 140, 140),
	inverse = Color3.fromRGB(0, 0, 0),
}

local function getSetting(key, fallback)
	local ok, value = pcall(function()
		return plugin:GetSetting(key)
	end)
	if ok and value ~= nil and value ~= "" then
		return value
	end
	return fallback
end

local function setSetting(key, value)
	pcall(function()
		plugin:SetSetting(key, value)
	end)
end

local state = {
	port = tonumber(getSetting("BloxCode.Port", DEFAULT_PORT)) or DEFAULT_PORT,
	token = getSetting("BloxCode.Token", DEFAULT_TOKEN),
	auto = getSetting("BloxCode.AutoSync", false) == true,
	running = false,
	lastRevision = -1,
}

local toolbar = plugin:CreateToolbar("BloxCode")
local toggleButton = toolbar:CreateButton(
	"BloxCode",
	"Sync AI-generated scripts into this place",
	"rbxasset://textures/ui/GuiImagePlaceholder.png"
)

local widgetInfo = DockWidgetPluginGuiInfo.new(Enum.InitialDockState.Right, false, false, 340, 430, 320, 380)
local widget = plugin:CreateDockWidgetPluginGui("BloxCodeBridge", widgetInfo)
widget.Title = "BloxCode"
widget.Name = "BloxCodeBridge"

local root = Instance.new("Frame")
root.BackgroundColor3 = theme.background
root.BorderSizePixel = 0
root.Size = UDim2.fromScale(1, 1)
root.Parent = widget

local padding = Instance.new("UIPadding")
padding.PaddingTop = UDim.new(0, 16)
padding.PaddingBottom = UDim.new(0, 16)
padding.PaddingLeft = UDim.new(0, 16)
padding.PaddingRight = UDim.new(0, 16)
padding.Parent = root

local layout = Instance.new("UIListLayout")
layout.FillDirection = Enum.FillDirection.Vertical
layout.SortOrder = Enum.SortOrder.LayoutOrder
layout.Padding = UDim.new(0, 8)
layout.Parent = root

local function label(text, size, color, order)
	local item = Instance.new("TextLabel")
	item.BackgroundTransparency = 1
	item.Font = Enum.Font.Gotham
	item.TextSize = size or 13
	item.TextColor3 = color or theme.text
	item.TextXAlignment = Enum.TextXAlignment.Left
	item.TextWrapped = true
	item.Text = text
	item.Size = UDim2.new(1, 0, 0, 0)
	item.AutomaticSize = Enum.AutomaticSize.Y
	item.LayoutOrder = order or 1
	item.Parent = root
	return item
end

label("BLOXCODE", 11, theme.muted, 1)
local title = label("Studio bridge", 20, theme.text, 2)

local status = label("Idle", 12, theme.muted, 3)

local function field(placeholder, value, order, onCommit)
	local box = Instance.new("TextBox")
	box.BackgroundColor3 = theme.panel
	box.BorderSizePixel = 0
	box.ClearTextOnFocus = false
	box.Font = Enum.Font.Code
	box.TextSize = 12
	box.TextColor3 = theme.text
	box.PlaceholderColor3 = theme.muted
	box.PlaceholderText = placeholder
	box.Text = value
	box.TextXAlignment = Enum.TextXAlignment.Left
	box.Size = UDim2.new(1, 0, 0, 30)
	box.LayoutOrder = order
	box.Parent = root

	local border = Instance.new("UIStroke")
	border.Color = theme.line
	border.Thickness = 1
	border.Parent = box

	local boxPadding = Instance.new("UIPadding")
	boxPadding.PaddingLeft = UDim.new(0, 8)
	boxPadding.PaddingRight = UDim.new(0, 8)
	boxPadding.Parent = box

	box.FocusLost:Connect(function()
		onCommit(box.Text)
	end)
	return box
end

local portBox
portBox = field("Port", tostring(state.port), 4, function(text)
	local port = tonumber(text)
	if port and port >= 1024 and port <= 65535 then
		state.port = port
		setSetting("BloxCode.Port", port)
		status.Text = "Port updated"
	else
		portBox.Text = tostring(state.port)
		status.Text = "Port must be 1024-65535"
	end
end)

local tokenBox
tokenBox = field("Bridge token", state.token, 5, function(text)
	if #text >= 16 then
		state.token = text
		setSetting("BloxCode.Token", text)
		status.Text = "Token updated"
	else
		tokenBox.Text = state.token
		status.Text = "Token looks truncated"
	end
end)

local syncButton = Instance.new("TextButton")
syncButton.BackgroundColor3 = theme.text
syncButton.BorderSizePixel = 0
syncButton.AutoButtonColor = false
syncButton.Font = Enum.Font.GothamBold
syncButton.TextSize = 13
syncButton.TextColor3 = theme.inverse
syncButton.Text = "SYNC NOW"
syncButton.Size = UDim2.new(1, 0, 0, 34)
syncButton.LayoutOrder = 6
syncButton.Parent = root

local autoButton = Instance.new("TextButton")
autoButton.BackgroundColor3 = theme.panel
autoButton.BorderSizePixel = 0
autoButton.AutoButtonColor = false
autoButton.Font = Enum.Font.Gotham
autoButton.TextSize = 12
autoButton.TextColor3 = theme.text
autoButton.Text = state.auto and "AUTO SYNC: ON" or "AUTO SYNC: OFF"
autoButton.Size = UDim2.new(1, 0, 0, 30)
autoButton.LayoutOrder = 7
autoButton.Parent = root

local autoStroke = Instance.new("UIStroke")
autoStroke.Color = theme.line
autoStroke.Thickness = 1
autoStroke.Parent = autoButton

local detail = label("Waiting for a project.", 12, theme.muted, 8)

local function request(path)
	local url = string.format("http://127.0.0.1:%d%s", state.port, path)
	local ok, response = pcall(function()
		return HttpService:RequestAsync({
			Url = url,
			Method = "GET",
			Headers = { ["X-BloxCode-Token"] = state.token },
		})
	end)
	if not ok then
		return nil, "HTTP requests are disabled. Enable Game Settings > Security > Allow HTTP Requests."
	end
	if not response.Success then
		if response.StatusCode == 401 then
			return nil, "Token rejected. Reinstall the plugin from BloxCode."
		end
		return nil, string.format("Bridge returned HTTP %d", response.StatusCode)
	end
	return response.Body
end

local function decode(body)
	local ok, value = pcall(function()
		return HttpService:JSONDecode(body)
	end)
	if not ok then
		return nil
	end
	return value
end

local function ensureFolder(parent, name)
	local existing = parent:FindFirstChild(name)
	if existing and existing:IsA("Folder") then
		return existing
	end
	local folder = Instance.new("Folder")
	folder.Name = name
	folder.Parent = parent
	return folder
end

local serviceRoots = {
	ReplicatedStorage = ReplicatedStorage,
	ServerScriptService = ServerScriptService,
	StarterPlayer = StarterPlayer,
}

local function resolveContainer(target)
	local segments = string.split(target, "/")
	local current = serviceRoots[segments[1]]
	if not current then
		return nil
	end
	for index = 2, #segments do
		current = ensureFolder(current, segments[index])
	end
	return current
end

local function beginRecording()
	local ok, recording = pcall(function()
		return ChangeHistoryService:TryBeginRecording("BloxCode sync", "BloxCode sync")
	end)
	if ok then
		return recording
	end
	pcall(function()
		ChangeHistoryService:SetWaypoint("BloxCode sync")
	end)
	return nil
end

local function finishRecording(recording)
	if recording then
		pcall(function()
			ChangeHistoryService:FinishRecording(recording, Enum.FinishRecordingOperation.Commit)
		end)
	end
end

local function sync()
	if state.running then
		return
	end
	state.running = true
	syncButton.Text = "SYNCING"
	status.Text = "Contacting BloxCode..."

	local manifestBody, manifestError = request("/v1/manifest")
	local manifest = manifestBody and decode(manifestBody)
	if not manifest then
		state.running = false
		syncButton.Text = "SYNC NOW"
		status.Text = "Bridge unavailable"
		detail.Text = manifestError or "Could not read the manifest."
		return
	end

	local recording = beginRecording()
	local expected = {}
	local written, created, failed = 0, 0, 0

	for _, file in ipairs(manifest.files or {}) do
		local body, fileError = request("/v1/file?path=" .. HttpService:UrlEncode(file.path))
		if body then
			local container = resolveContainer(file.target)
			if container then
				local instance = container:FindFirstChild(file.name)
				if instance and instance.ClassName ~= file.class then
					instance:Destroy()
					instance = nil
				end
				if not instance then
					instance = Instance.new(file.class)
					instance.Name = file.name
					instance.Parent = container
					created += 1
				end
				if SCRIPT_CLASSES[instance.ClassName] and instance.Source ~= body then
					instance.Source = body
					written += 1
				end
				expected[container] = expected[container] or {}
				table.insert(expected[container], file.name)
			else
				failed += 1
			end
		else
			failed += 1
			detail.Text = fileError or "Could not read a file."
		end
	end

	-- Remove scripts BloxCode previously managed but no longer ships.
	for container, names in pairs(expected) do
		for _, child in ipairs(container:GetChildren()) do
			if SCRIPT_CLASSES[child.ClassName] and not table.find(names, child.Name) then
				child:Destroy()
			end
		end
	end

	local remotes = resolveContainer("ReplicatedStorage/BloxCode/Remotes")
	if remotes then
		for _, remote in ipairs(manifest.remotes or {}) do
			if not remotes:FindFirstChild(remote.name) then
				local instance = Instance.new(remote.class)
				instance.Name = remote.name
				instance.Parent = remotes
			end
		end
	end

	finishRecording(recording)
	state.lastRevision = tonumber(manifest.revision) or state.lastRevision
	state.running = false
	syncButton.Text = "SYNC NOW"
	status.Text = string.format("Synced revision %s", tostring(manifest.revision))
	detail.Text = string.format(
		"%d files updated, %d created, %d failed. %s",
		written,
		created,
		failed,
		manifest.title or ""
	)
end

syncButton.MouseButton1Click:Connect(function()
	task.spawn(sync)
end)

autoButton.MouseButton1Click:Connect(function()
	state.auto = not state.auto
	setSetting("BloxCode.AutoSync", state.auto)
	autoButton.Text = state.auto and "AUTO SYNC: ON" or "AUTO SYNC: OFF"
	status.Text = state.auto and "Watching for changes" or "Auto sync off"
end)

toggleButton.Click:Connect(function()
	widget.Enabled = not widget.Enabled
end)

task.spawn(function()
	while true do
		task.wait(state.auto and POLL_SECONDS or 2)
		if state.auto and widget.Enabled and not state.running then
			local ok, err = pcall(sync)
			if not ok then
				status.Text = "Sync error"
				detail.Text = tostring(err)
				state.running = false
				syncButton.Text = "SYNC NOW"
			end
		end
	end
end)

status.Text = "Ready"
detail.Text = "Open a project in BloxCode, then press Sync Now."
