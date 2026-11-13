-- Wireshark Lua dissector for the rperf protocol.
--
-- rperf (https://github.com/opensource-3d-p/rperf) is an iperf3-alternative
-- for measuring TCP/UDP throughput and quality.

-- Note that while UDP auto-detection should work pretty consistently, TCP auto-detection
-- is not possible with the simplicity of the protocol. You will need to use "Decode As"
-- to see any TCP packet information. Most of the interesting details are in UDP, though.


local rperf = Proto("rperf", "rperf Network Performance Test")
local rperf_data = Proto("rperf.data", "rperf TCP Test Data")

-- ---------------------------------------------------------------------------
-- Constants
-- ---------------------------------------------------------------------------
local PROTOCOL_NAME = "rperf"
local TEST_ID_LEN = 16

local UDP_DATA_HEADER_LEN = 36
local TRACER_SIZE = 21
local TRACER_MAGIC = "RNAT"

local TCP_DATA_HEADER_LEN = 16

local DEFAULT_CONTROL_PORT = 5199

local ZERO_TEST_ID = string.rep("\0", TEST_ID_LEN)

-- ---------------------------------------------------------------------------
-- ProtoFields
-- ---------------------------------------------------------------------------

-- Control channel
local f_msg_length = ProtoField.uint16("rperf.control.length", "Message Length", base.DEC)
local f_payload = ProtoField.none("rperf.control.payload", "Payload (JSON)")

-- UDP tracer datagrams
local f_tracer_magic = ProtoField.bytes("rperf.tracer.magic", "Magic (\"RNAT\")")
local f_tracer_test_id = ProtoField.guid("rperf.tracer.test_id", "Test ID")
local f_tracer_stream = ProtoField.uint8("rperf.tracer.stream_idx", "Stream Index", base.DEC)

-- UDP test data
local f_data_test_id = ProtoField.guid("rperf.udp.test_id", "Test ID")
local f_data_packet_id = ProtoField.uint64("rperf.udp.packet_id", "Packet ID", base.DEC)
local f_data_origin_seconds = ProtoField.uint64("rperf.udp.origin_seconds", "Origin Seconds", base.DEC)
local f_data_origin_nanoseconds = ProtoField.uint32("rperf.udp.origin_nanoseconds", "Origin Nanoseconds", base.DEC)
local f_data_origin_ts = ProtoField.string("rperf.udp.origin_timestamp", "Origin Timestamp")
local f_data_payload = ProtoField.none("rperf.udp.payload", "Test Data")
local f_data_ordering = ProtoField.string("rperf.udp.ordering", "Ordering")
local f_data_gap_frame = ProtoField.framenum("rperf.udp.gap_frame", "Gap opened in frame", base.NONE)
local f_data_end = ProtoField.none("rperf.udp.end_of_test", "End of Test")

-- TCP test data
local f_tcp_block = ProtoField.none("rperf.data.block", "Test Data")
local f_tcp_block_length = ProtoField.uint32("rperf.data.block_length", "Inferred Length", base.DEC)

rperf.fields = {
  f_msg_length, f_payload,
  f_data_test_id, f_data_packet_id,
  f_data_origin_seconds, f_data_origin_nanoseconds, f_data_origin_ts,
  f_data_payload, f_data_ordering, f_data_gap_frame, f_data_end,
  f_tracer_magic, f_tracer_test_id, f_tracer_stream,
}

rperf_data.fields = {
  f_tcp_block, f_tcp_block_length,
}

-- ---------------------------------------------------------------------------
-- Expert info (ProtoExpert)
-- ---------------------------------------------------------------------------
local ei_partial = ProtoExpert.new("rperf.partial", "rperf control message incomplete in this segment", expert.group.MALFORMED, expert.severity.WARN)

local ei_data_ooo = ProtoExpert.new("rperf.udp.out_of_order", "Out-of-order / duplicate packet ID in rperf UDP stream", expert.group.SEQUENCE, expert.severity.WARN)
local ei_udp_pattern = ProtoExpert.new("rperf.udp.pattern", "rperf UDP payload does not match the expected i%256 byte pattern", expert.group.MALFORMED, expert.severity.WARN)

local ei_tcp_zero_id = ProtoExpert.new("rperf.data.zero_test_id", "rperf TCP data stream has an all-zero Test ID", expert.group.MALFORMED, expert.severity.WARN)
local ei_tcp_pattern = ProtoExpert.new("rperf.data.pattern", "rperf TCP data payload does not match the expected byte pattern", expert.group.MALFORMED, expert.severity.WARN)

rperf.experts = {
  ei_partial,
  ei_data_ooo,
  ei_udp_pattern,
}

rperf_data.experts = {
  ei_tcp_zero_id,
  ei_tcp_pattern,
}

-- State table for packet-ordering tracking of UDP test data.
local udp_flow_state = {}
-- Per-frame out-of-order status.
local udp_ooo_by_frame = {}
-- Window (in packet IDs) kept per flow to distinguish a true duplicate.
local OOO_SEEN_WINDOW = 1024

function rperf.init()
  udp_flow_state = {}
  udp_ooo_by_frame = {}
end

-- ---------------------------------------------------------------------------
-- Helpers
-- ---------------------------------------------------------------------------
local function be64_to_number(range)
  return range(0, 4):uint() * 4294967296 + range(4, 4):uint()
end

local function tvb_bytes(tvb, offset, length)
  local bytes = {}
  for i = 0, length - 1 do
    bytes[i + 1] = string.char(tvb(offset + i, 1):uint())
  end
  return table.concat(bytes)
end

local function format_test_id_uuid(raw)
  return string.format(
    "%02x%02x%02x%02x-%02x%02x-%02x%02x-%02x%02x-%02x%02x%02x%02x%02x%02x",
    raw:byte(1), raw:byte(2), raw:byte(3), raw:byte(4),
    raw:byte(5), raw:byte(6), raw:byte(7), raw:byte(8),
    raw:byte(9), raw:byte(10), raw:byte(11), raw:byte(12),
    raw:byte(13), raw:byte(14), raw:byte(15), raw:byte(16)
    )
end

local function format_origin_timestamp(seconds, nanos)
  local ok, dt = pcall(os.date, "!%Y-%m-%d %H:%M:%S", math.floor(seconds))
  if ok and dt then
    return string.format("%s.%09d UTC", dt, nanos)
  end
  return string.format("%d.%09d (UNIX)", seconds, nanos)
end

local json_dissector = Dissector.get("json")

-- ---------------------------------------------------------------------------
-- TCP test-data
-- ---------------------------------------------------------------------------
local function looks_like_tcp_data(buffer)
  local len = buffer:len()
  if len < TCP_DATA_HEADER_LEN then
    return false
  end

  if len > TCP_DATA_HEADER_LEN and tvb_bytes(buffer, 0, TEST_ID_LEN) ~= ZERO_TEST_ID then
    local seq_ok = true
    for i = TCP_DATA_HEADER_LEN, len - 1 do
      if buffer(i, 1):uint() ~= (i - TCP_DATA_HEADER_LEN) % 256 then
        seq_ok = false
        break
      end
    end
    if seq_ok then
      return true
    end
  end

  local raw = tvb_bytes(buffer, 0, len)
  if string.find(raw, string.char(16, 17, 18, 19), 1, true) then
    return true
  end

  local sequential = true
  for i = 1, len - 1 do
    if buffer(i, 1):uint() ~= (buffer(i - 1, 1):uint() + 1) % 256 then
      sequential = false
      break
    end
  end
  return sequential
end

-- ---------------------------------------------------------------------------
-- Control channel dissector
-- ---------------------------------------------------------------------------
function rperf.dissector(buffer, pinfo, tree)
  local len = buffer:len()
  if len < 2 then
    return 0
  end

  if looks_like_tcp_data(buffer) then
    return rperf_data.dissector(buffer, pinfo, tree)
  end

  pinfo.cols.protocol = PROTOCOL_NAME

  local offset = 0
  local messages = 0
  while offset + 2 <= len do
    local msg_len = buffer(offset, 2):uint()
    if offset + 2 + msg_len > len then
      local partial_item = tree:add(rperf, buffer(offset, len - offset), "rperf control (incomplete)")
      partial_item:add(f_msg_length, buffer(offset, 2))
      if msg_len > 0 then
        partial_item:add(f_payload, buffer(offset + 2, len - offset - 2))
      end
      partial_item:add_proto_expert_info(ei_partial,
        string.format("Message is %d bytes long, but only %d are available in this segment",
          msg_len, len - offset - 2))
      offset = len
    else
      local msg_item = tree:add(rperf, buffer(offset, 2 + msg_len),
        "rperf control message (" .. tostring(msg_len) .. " bytes)")
      msg_item:add(f_msg_length, buffer(offset, 2))

      local payload_item = msg_item:add(f_payload, buffer(offset + 2, msg_len))

      if json_dissector then
        pcall(function()
          payload_item:add_item(json_dissector, buffer(offset + 2, msg_len))
        end)
      end

      messages = messages + 1
      offset = offset + 2 + msg_len
    end
  end

  if messages == 1 then
    pinfo.cols.info = "rperf control message"
  else
    pinfo.cols.info = tostring(messages) .. " rperf control messages"
  end
  return len
end

-- ---------------------------------------------------------------------------
-- UDP test data
-- ---------------------------------------------------------------------------
local function dissect_end_of_test(buffer, pinfo, tree_item)
  pinfo.cols.protocol = PROTOCOL_NAME

  local test_id_raw = tvb_bytes(buffer, 0, TEST_ID_LEN)
  tree_item:add(f_data_end, buffer(0, TEST_ID_LEN))
  tree_item:add(f_data_test_id, buffer(0, TEST_ID_LEN))
  pinfo.cols.info = "rperf UDP data: end of test (test " .. format_test_id_uuid(test_id_raw) .. ")"
end

local function dissect_udp_data(buffer, pinfo, tree_item)
  local len = buffer:len()

  local test_id_raw = tvb_bytes(buffer, 0, TEST_ID_LEN)
  local packet_id = be64_to_number(buffer(TEST_ID_LEN, 8))
  local origin_seconds = be64_to_number(buffer(24, 8))
  local origin_nanoseconds = buffer(32, 4):uint()

  tree_item:add(f_data_test_id, buffer(0, TEST_ID_LEN))
  tree_item:add(f_data_packet_id, buffer(TEST_ID_LEN, 8))
  tree_item:add(f_data_origin_seconds, buffer(24, 8))
  tree_item:add(f_data_origin_nanoseconds, buffer(32, 4))

  local ts_item = tree_item:add(f_data_origin_ts,
    format_origin_timestamp(origin_seconds, origin_nanoseconds))
  ts_item:set_generated()

  if len > UDP_DATA_HEADER_LEN then
    local payload_item = tree_item:add(f_data_payload, buffer(UDP_DATA_HEADER_LEN, len - UDP_DATA_HEADER_LEN))
    payload_item:append_text(string.format(" (%d bytes)", len - UDP_DATA_HEADER_LEN))
    for off = UDP_DATA_HEADER_LEN, len - 1 do
      local expected = off % 256
      if buffer(off, 1):uint() ~= expected then
        payload_item:add_proto_expert_info(ei_udp_pattern,
          string.format("Payload byte at offset %d is 0x%02x, expected 0x%02x (i%%256 fill)",
            off, buffer(off, 1):uint(), expected))
        break
      end
    end
  end

  local ooo
  if not pinfo.visited then
    local flow_key = tostring(pinfo.src) .. "->" .. tostring(pinfo.dst) .. ":" .. format_test_id_uuid(test_id_raw)
    local flow = udp_flow_state[flow_key]
    if not flow then
      flow = {
        hw = packet_id - 1,
        seen = {},
        seen_count = 0,
        max_seen = packet_id,
        last_received = packet_id,
        gap_frames = {},
      }
      udp_flow_state[flow_key] = flow
    elseif flow.seen[packet_id] then
      ooo = {
        text = "duplicate (already received)",
        msg = string.format("Duplicate packet ID %d (already received)", packet_id),
      }
    elseif (packet_id == flow.hw + 1) or (packet_id < flow.max_seen and flow.gap_frames[packet_id]) then
      local gap_frame = flow.gap_frames[packet_id]
      if gap_frame then
        ooo = {
          text = string.format("follows gap at frame %d", gap_frame),
          msg = string.format("Packet ID %d follows gap at frame %d",
            packet_id, gap_frame),
          gap_frame = gap_frame,
        }
      end
    elseif packet_id > flow.max_seen and packet_id ~= flow.last_received + 1 then
      local gap_size = packet_id - flow.last_received - 1
      if gap_size <= OOO_SEEN_WINDOW then
        for id = flow.last_received + 1, packet_id - 1 do
          flow.gap_frames[id] = pinfo.number
        end
      end
      ooo = {
        text = string.format("gap of %d", gap_size),
        msg = string.format("Packet ID %d received, expected %d (gap of %d)",
          packet_id, flow.last_received + 1, gap_size),
        gap_frame = pinfo.number,
      }
    end

    if not flow.seen[packet_id] then
      flow.seen[packet_id] = true
      flow.seen_count = flow.seen_count + 1
    end

    flow.gap_frames[packet_id] = nil
    if packet_id > flow.max_seen then
      flow.max_seen = packet_id
    end
    flow.last_received = packet_id
    while flow.seen[flow.hw + 1] do
      flow.hw = flow.hw + 1
    end
    if flow.seen_count > OOO_SEEN_WINDOW * 2 then
      local cutoff = flow.hw + 1
      for id in pairs(flow.seen) do
        if id < cutoff then
          flow.seen[id] = nil
          flow.seen_count = flow.seen_count - 1
        end
      end
    end
    udp_ooo_by_frame[pinfo.number] = ooo
  else
    ooo = udp_ooo_by_frame[pinfo.number]
  end

  if ooo then
    local order_item = tree_item:add(f_data_ordering, "out of order: " .. ooo.text)
    order_item:set_generated()
    order_item:add_proto_expert_info(ei_data_ooo, ooo.msg)
    if ooo.gap_frame then
      order_item:add(f_data_gap_frame, ooo.gap_frame):set_generated()
    end
  end

  local info = "rperf UDP data: packet #" .. string.format("%d", packet_id) .. " (test " .. format_test_id_uuid(test_id_raw) .. ")"
  if ooo then
    info = info .. " [out of order: " .. ooo.text .. "]"
  end
  pinfo.cols.info = info
end

local function dissect_tracer(buffer, pinfo, tree_item)
  tree_item:add(f_tracer_magic, buffer(0, 4))
  tree_item:add(f_tracer_test_id, buffer(4, TEST_ID_LEN))

  local stream_idx = buffer(20, 1):uint()
  tree_item:add(f_tracer_stream, buffer(20, 1))

  pinfo.cols.info = "rperf UDP tracer (NAT resolution): stream " .. tostring(stream_idx)
end

local function dissect_udp(buffer, pinfo, tree)
  pinfo.cols.protocol = PROTOCOL_NAME
  local len = buffer:len()

  if len == TRACER_SIZE then
    local tree_item = tree:add(rperf, buffer(), "rperf UDP tracer")
    dissect_tracer(buffer, pinfo, tree_item)
    return len
  end

  if len < UDP_DATA_HEADER_LEN then
    return 0
  end

  local tree_item = tree:add(rperf, buffer(), "rperf UDP test data")
  dissect_udp_data(buffer, pinfo, tree_item)
  return len
end

-- ---------------------------------------------------------------------------
-- TCP test data
-- ---------------------------------------------------------------------------
function rperf_data.dissector(buffer, pinfo, tree)
  local len = buffer:len()
  if len < TCP_DATA_HEADER_LEN then
    return 0
  end

  pinfo.cols.protocol = "RPERF DATA"
  local tree_item = tree:add(rperf_data, buffer(), "rperf TCP test data")

  local raw = tvb_bytes(buffer, 0, len)
  local fill_sig = string.char(16, 17, 18, 19)
  local q = string.find(raw, fill_sig, 1, true)
  local first_start = nil
  if q then
    first_start = q - TEST_ID_LEN - 1
    if first_start < 0 then
      first_start = 0
    end
  end

  if not first_start then
    local sequential = len >= 2
    for i = 1, len - 1 do
      if buffer(i, 1):uint() ~= (buffer(i - 1, 1):uint() + 1) % 256 then
        sequential = false
        break
      end
    end
    if sequential then
      local item = tree_item:add(f_tcp_block, buffer(0, len))
      item:append_text(string.format(" (Partial Block, fill continuation - no boundary in segment, %d bytes)", len))
      pinfo.cols.info = "rperf TCP data: fill continuation (" .. tostring(len) .. " bytes)"
      return len
    end

    local unknown_item = tree_item:add(f_tcp_block, buffer(0, len))
    unknown_item:append_text(" (No Block Boundary Found)")
    tree_item:add_proto_expert_info(ei_tcp_pattern,
      "No block boundary (Test ID + i%256 fill) found in this segment")
    pinfo.cols.info = "rperf TCP data (no block boundary found)"
    return len
  end

  local test_id_raw = tvb_bytes(buffer, first_start, TEST_ID_LEN)
  if test_id_raw == ZERO_TEST_ID then
    local zero_item = tree_item:add(f_tcp_block, buffer(0, len))
    zero_item:append_text(" (zero Test ID)")
    tree_item:add_proto_expert_info(ei_tcp_zero_id, "Test ID at the block boundary is all zero; not a rperf test stream?")
    pinfo.cols.info = "rperf TCP data (zero Test ID)"
    return len
  end

  if first_start > 0 then
    local partial_item = tree_item:add(f_tcp_block, buffer(0, first_start))
    partial_item:append_text(string.format(" (Partial Block, segment starts mid-block, %d bytes)", first_start))
  end

  local offsets = { first_start + 1 }
  local pos = first_start + TEST_ID_LEN + 1
  while #offsets < 64 do
    local start = string.find(raw, test_id_raw, pos, true)
    if not start then break end
    table.insert(offsets, start)
    pos = start + TEST_ID_LEN
  end

  local inferred = nil
  if #offsets > 1 then
    inferred = offsets[2] - offsets[1]
    local inf_item = tree_item:add(f_tcp_block_length, inferred)
    inf_item:set_generated()
  end

  for i = 1, #offsets do
    local start_off = offsets[i] - 1 --0-based
    local end_off = (i < #offsets) and (offsets[i + 1] - 1) or (len - 1)
    local block_len = end_off - start_off + 1
    local note = "Test Data Block"
    if inferred and block_len < inferred then
      note = note .. " (truncated by segment)"
    end
    local block_item = tree_item:add(f_tcp_block, buffer(start_off, block_len))
    block_item:append_text(string.format(" (%s, %d bytes)", note, block_len))
  end

  local info_parts = { "rperf TCP data (test " .. format_test_id_uuid(test_id_raw) .. ", " .. tostring(len) .. " bytes)" }
  if inferred then
    table.insert(info_parts, "block length " .. tostring(inferred) .. " (inferred)")
  end
  pinfo.cols.info = table.concat(info_parts, ", ")
  return len
end

-- ---------------------------------------------------------------------------
-- Heuristic: recognise rperf UDP packets on any port
-- ---------------------------------------------------------------------------
local function rperf_udp_heuristic(buffer, pinfo, tree)
  local len = buffer:len()

  if len == TRACER_SIZE then
    if tvb_bytes(buffer, 0, 4) ~= TRACER_MAGIC then
      return false
    end
    dissect_udp(buffer, pinfo, tree)
    return true
  end

  if len == TEST_ID_LEN then
    local flow_key = tostring(pinfo.src) .. "->" .. tostring(pinfo.dst) .. ":"
      .. format_test_id_uuid(tvb_bytes(buffer, 0, TEST_ID_LEN))
    if udp_flow_state[flow_key] then
      dissect_end_of_test(buffer, pinfo, tree)
      return true
    end
    return false
  end

  if len < UDP_DATA_HEADER_LEN then
    return false
  end
  if tvb_bytes(buffer, 0, 4) == TRACER_MAGIC then
    return false
  end
  if tvb_bytes(buffer, 0, TEST_ID_LEN) == ZERO_TEST_ID then
    return false
  end

  for i = UDP_DATA_HEADER_LEN, math.min(UDP_DATA_HEADER_LEN + 3, len - 1) do
    if buffer(i, 1):uint() ~= (i % 256) then
      return false
    end
  end

  dissect_udp(buffer, pinfo, tree)
  return true
end

-- ---------------------------------------------------------------------------
-- Registration
-- ---------------------------------------------------------------------------
local tcp_table = DissectorTable.get("tcp.port")
tcp_table:add(DEFAULT_CONTROL_PORT, rperf)

rperf:register_heuristic("udp", rperf_udp_heuristic)
