import { useCallback, useEffect, useRef, useState } from 'react';
import type { FormEvent, KeyboardEvent } from 'react';

interface ChatMessage {
  type: 'message';
  id: string;
  nickname: string;
  text: string;
  message_kind?: 'chat' | 'social';
  action_id?: string;
  sent_at: string;
}

interface ChatTicket {
  ticket: string;
  nickname: string;
  expires_in_seconds: number;
}

interface ChatCommandDefinition {
  command: string;
  response: string;
}

type ConnectionStatus = 'connecting' | 'connected' | 'reconnecting';

const MAX_MESSAGES = 100;
const MAX_MESSAGE_CHARACTERS = 500;
const INITIAL_RECONNECT_DELAY_MS = 1_000;
const MAX_RECONNECT_DELAY_MS = 30_000;

let cachedChatCommands: ChatCommandDefinition[] | null = null;
let chatCommandsRequest: Promise<ChatCommandDefinition[]> | null = null;

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : '알 수 없는 오류가 발생했습니다.';
}

function isChatCommandDefinition(value: unknown): value is ChatCommandDefinition {
  if (typeof value !== 'object' || value === null) return false;
  const command = value as Record<string, unknown>;
  return typeof command.command === 'string' && typeof command.response === 'string';
}

function isChatCommandList(value: unknown): value is ChatCommandDefinition[] {
  return Array.isArray(value) && value.every(isChatCommandDefinition);
}

function loadChatCommands(): Promise<ChatCommandDefinition[]> {
  if (cachedChatCommands) return Promise.resolve(cachedChatCommands);
  if (chatCommandsRequest) return chatCommandsRequest;

  const request = (async () => {
    const response = await fetch('/api/chat/commands');
    if (!response.ok) {
      throw new Error(`명령어 목록 요청 실패 (HTTP ${response.status})`);
    }

    const commandData: unknown = await response.json();
    if (!isChatCommandList(commandData)) {
      throw new Error('채팅 서버가 올바르지 않은 명령어 목록을 반환했습니다.');
    }

    cachedChatCommands = commandData;
    return commandData;
  })();
  chatCommandsRequest = request;
  void request.then(
    () => {
      if (chatCommandsRequest === request) chatCommandsRequest = null;
    },
    () => {
      if (chatCommandsRequest === request) chatCommandsRequest = null;
    },
  );
  return request;
}

function isChatTicket(value: unknown): value is ChatTicket {
  if (typeof value !== 'object' || value === null) return false;
  const ticket = value as Record<string, unknown>;
  return (
    typeof ticket.ticket === 'string' &&
    typeof ticket.nickname === 'string' &&
    typeof ticket.expires_in_seconds === 'number'
  );
}

function isChatMessage(value: unknown): value is ChatMessage {
  if (typeof value !== 'object' || value === null) return false;
  const message = value as Record<string, unknown>;
  return (
    message.type === 'message' &&
    typeof message.id === 'string' &&
    typeof message.nickname === 'string' &&
    typeof message.text === 'string' &&
    (message.message_kind === undefined ||
      message.message_kind === 'chat' ||
      message.message_kind === 'social') &&
    (message.action_id === undefined || typeof message.action_id === 'string') &&
    typeof message.sent_at === 'string'
  );
}

function getChatWebSocketUrl(ticket: string): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = new URL(`${protocol}//${window.location.host}/ws/chat`);
  url.searchParams.set('ticket', ticket);
  return url.toString();
}

function formatTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleTimeString();
}

const SOCIAL_ACTION_EMOJI: Record<string, string> = {
  go: '🚶',
  come: '👋',
  stand: '🧍',
  sit: '🪑',
  lie_down: '🛏️',
  get_up: '🌅',
  return_go: '↩️',
  return_come: '🏃',
  exit: '🚪',
  enter: '🚪',
  come_inside: '🏠',
  run: '🏃',
  walk: '🚶',
  laugh: '😄',
  cry: '😢',
  eat: '🍽️',
  drink: '🥤',
  sleep: '😴',
  rest: '☕',
  dance: '💃',
  sing: '🎵',
  applause: '👏',
};

function usePublicChat(enabled: boolean) {
  const [, setStatus] = useState<ConnectionStatus>('connecting');
  const [nickname, setNickname] = useState('');
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [connectionError, setConnectionError] = useState('');
  const [, setConnectionClosed] = useState(false);
  const [sendError, setSendError] = useState('');
  const socketRef = useRef<WebSocket | null>(null);
  // StrictMode replays effects in development; share the pending POST to avoid burning an alias.
  const ticketRequestRef = useRef<Promise<ChatTicket> | null>(null);

  useEffect(() => {
    if (!enabled) {
      setStatus('connecting');
      return;
    }

    let disposed = false;
    let connectedBefore = false;
    let connecting = false;
    let reconnectDelay = INITIAL_RECONNECT_DELAY_MS;
    let reconnectTimer: number | null = null;
    let socket: WebSocket | null = null;

    const requestTicket = () => {
      if (ticketRequestRef.current) return ticketRequestRef.current;

      const request = (async () => {
        const response = await fetch('/api/chat/ticket', { method: 'POST' });
        if (!response.ok) {
          throw new Error(`티켓 발급 실패 (HTTP ${response.status})`);
        }

        const ticketData: unknown = await response.json();
        if (!isChatTicket(ticketData)) {
          throw new Error('채팅 서버가 올바르지 않은 연결 티켓을 반환했습니다.');
        }
        return ticketData;
      })();
      ticketRequestRef.current = request;
      void request.then(
        () => {
          if (ticketRequestRef.current === request) ticketRequestRef.current = null;
        },
        () => {
          if (ticketRequestRef.current === request) ticketRequestRef.current = null;
        },
      );
      return request;
    };

    const scheduleReconnect = () => {
      if (disposed || !enabled || reconnectTimer !== null) return;
      setStatus('reconnecting');
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null;
        void connect();
      }, reconnectDelay);
      reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_DELAY_MS);
    };

    const connect = async () => {
      if (disposed || !enabled || connecting) return;
      connecting = true;
      if (connectedBefore) setStatus('reconnecting');

      try {
        const ticketData = await requestTicket();
        if (disposed) return;

        setNickname(ticketData.nickname);
        const nextSocket = new WebSocket(getChatWebSocketUrl(ticketData.ticket));
        socket = nextSocket;
        socketRef.current = nextSocket;

        nextSocket.onopen = () => {
          if (disposed) {
            nextSocket.close();
            return;
          }
          connectedBefore = true;
          reconnectDelay = INITIAL_RECONNECT_DELAY_MS;
          setConnectionClosed(false);
          setStatus('connected');
          setConnectionError('');
        };

        nextSocket.onmessage = (event: MessageEvent<unknown>) => {
          if (disposed) return;
          try {
            if (typeof event.data !== 'string') {
              throw new Error('채팅 서버에서 지원하지 않는 메시지 형식을 받았습니다.');
            }
            const data: unknown = JSON.parse(event.data);
            if (isChatMessage(data)) {
              setMessages((current) => [...current, data].slice(-MAX_MESSAGES));
              return;
            }
            if (
              typeof data === 'object' &&
              data !== null &&
              'type' in data &&
              data.type === 'error'
            ) {
              const code =
                'code' in data && typeof data.code === 'string' ? data.code : 'unknown';
              setConnectionError(`채팅 서버 오류: ${code}`);
              return;
            }
            throw new Error('채팅 서버에서 올바르지 않은 메시지를 받았습니다.');
          } catch (error) {
            setConnectionError(getErrorMessage(error));
          }
        };

        nextSocket.onerror = () => {
          if (disposed) return;
          setConnectionError('채팅 연결에 문제가 있습니다. 다시 연결합니다.');
          nextSocket.close();
        };

        nextSocket.onclose = () => {
          if (disposed || socketRef.current !== nextSocket) return;
          socketRef.current = null;
          socket = null;
          setConnectionClosed(true);
          scheduleReconnect();
        };
      } catch (error) {
        if (!disposed) {
          setConnectionError(getErrorMessage(error));
          scheduleReconnect();
        }
      } finally {
        connecting = false;
      }
    };

    void connect();
    return () => {
      disposed = true;
      if (reconnectTimer !== null) window.clearTimeout(reconnectTimer);
      socketRef.current = null;
      socket?.close();
    };
  }, [enabled]);

  const sendMessage = useCallback((text: string): boolean => {
    const trimmed = text.trim();
    if (!trimmed) return false;
    if (Array.from(trimmed).length > MAX_MESSAGE_CHARACTERS) {
      setSendError(`메시지는 ${MAX_MESSAGE_CHARACTERS}자까지 입력할 수 있습니다.`);
      return false;
    }

    const socket = socketRef.current;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      setSendError('채팅 서버와 연결되어 있지 않습니다.');
      return false;
    }

    try {
      socket.send(JSON.stringify({ type: 'send', text: trimmed }));
      setSendError('');
      return true;
    } catch (error) {
      setSendError(`메시지 전송 실패: ${getErrorMessage(error)}`);
      return false;
    }
  }, []);

  return {
    connectionError,
    messages,
    nickname,
    sendError,
    sendMessage,
    setSendError,
  };
}

export default function ChatPage({ active, backendReady }: { active: boolean; backendReady: boolean }) {
  const {
    connectionError,
    messages,
    nickname,
    sendError,
    sendMessage,
    setSendError,
  } = usePublicChat(backendReady);
  const [text, setText] = useState('');
  const [commands, setCommands] = useState<ChatCommandDefinition[]>([]);
  const [commandLoadError, setCommandLoadError] = useState('');
  const [commandPickerDismissed, setCommandPickerDismissed] = useState(false);
  const [activeCommandIndex, setActiveCommandIndex] = useState(0);
  const messageListRef = useRef<HTMLOListElement>(null);
  const messageInputRef = useRef<HTMLInputElement>(null);
  const commandOptionRefs = useRef<Array<HTMLButtonElement | null>>([]);

  useEffect(() => {
    if (!active) return;
    const list = messageListRef.current;
    if (list) list.scrollTop = list.scrollHeight;
  }, [active, messages]);

  useEffect(() => {
    if (!backendReady) return;
    let disposed = false;
    void loadChatCommands().then(
      (loadedCommands) => {
        if (!disposed) setCommands(loadedCommands);
      },
      (error: unknown) => {
        if (!disposed) {
          setCommandLoadError(
            `명령어 목록을 불러오지 못했습니다: ${getErrorMessage(error)}`,
          );
        }
      },
    );
    return () => {
      disposed = true;
    };
  }, [backendReady]);

  const commandSuggestions =
    !commandPickerDismissed && text.startsWith('/')
      ? commands.filter(({ command }) => command.startsWith(text))
      : [];
  const activeSuggestionIndex = Math.min(
    activeCommandIndex,
    Math.max(commandSuggestions.length - 1, 0),
  );

  useEffect(() => {
    if (!commandSuggestions.length) return;
    commandOptionRefs.current[activeSuggestionIndex]?.scrollIntoView({ block: 'nearest' });
  }, [activeSuggestionIndex, commandSuggestions.length]);

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (sendMessage(text)) setText('');
  };

  const selectCommand = (command: ChatCommandDefinition) => {
    const sent = sendMessage(command.command);
    setText(sent ? '' : command.command);
    setCommandPickerDismissed(true);
    setActiveCommandIndex(0);
    messageInputRef.current?.focus();
  };

  const handleComposerKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (!commandSuggestions.length) {
      if (event.key === 'Escape' && text.startsWith('/')) {
        setCommandPickerDismissed(true);
      }
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActiveCommandIndex((index) => (index + 1) % commandSuggestions.length);
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActiveCommandIndex(
        (index) => (index - 1 + commandSuggestions.length) % commandSuggestions.length,
      );
    } else if (event.key === 'Enter') {
      event.preventDefault();
      selectCommand(commandSuggestions[activeSuggestionIndex]);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      setCommandPickerDismissed(true);
    }
  };

  return (
    <main className="chat-page">
      {connectionError && (
        <p className="chat-error" role="alert">
          {connectionError}
        </p>
      )}

      <ol className="chat-messages" aria-live="polite" ref={messageListRef}>
        {messages.length === 0 ? (
          <li className="chat-empty">새 메시지를 기다리는 중입니다.</li>
        ) : (
          messages.map((message, index) => {
            const isContinuation =
              index > 0 && messages[index - 1]?.nickname === message.nickname;

            return (
              <li
                className={`chat-message${isContinuation ? ' chat-message-continuation' : ''}${message.message_kind === 'social' ? ' chat-message-social' : ''}`}
                key={message.id}
              >
                {!isContinuation && (
                  <div className="chat-message-meta">
                    <strong>{message.nickname}</strong>
                  </div>
                )}
                <div className="chat-message-row">
                  <p>
                    {message.text}
                    {message.message_kind === 'social' && message.action_id && (
                      <span className="chat-social-emoji" aria-hidden="true">
                        {' '}{SOCIAL_ACTION_EMOJI[message.action_id] ?? '✨'}
                      </span>
                    )}
                  </p>
                  <time dateTime={message.sent_at}>{formatTime(message.sent_at)}</time>
                </div>
              </li>
            );
          })
        )}
      </ol>

      <form className="chat-composer" onSubmit={handleSubmit}>
        {commandSuggestions.length > 0 && (
          <div
            className="chat-command-picker"
            id="chat-command-picker"
            role="listbox"
            aria-label="채팅 명령어"
          >
            {commandSuggestions.map((command, index) => (
              <button
                aria-selected={index === activeSuggestionIndex}
                className="chat-command-option"
                id={`chat-command-option-${index}`}
                key={command.command}
                onClick={() => selectCommand(command)}
                onMouseEnter={() => setActiveCommandIndex(index)}
                onMouseDown={(event) => event.preventDefault()}
                ref={(element) => {
                  commandOptionRefs.current[index] = element;
                }}
                role="option"
                type="button"
              >
                <strong>{command.command}</strong>
                <span>{command.response}</span>
              </button>
            ))}
          </div>
        )}
        <span className="chat-composer-nickname">
          {nickname || '닉네임 발급 중...'}
        </span>
        <label className="chat-visually-hidden" htmlFor="chat-message-input">
          메시지
        </label>
        <input
          aria-activedescendant={
            commandSuggestions.length
              ? `chat-command-option-${activeSuggestionIndex}`
              : undefined
          }
          aria-autocomplete="list"
          aria-controls={commandSuggestions.length ? 'chat-command-picker' : undefined}
          aria-expanded={commandSuggestions.length > 0}
          id="chat-message-input"
          onKeyDown={handleComposerKeyDown}
          value={text}
          onChange={(event) => {
            setText(event.target.value);
            setCommandPickerDismissed(false);
            setActiveCommandIndex(0);
            setSendError('');
          }}
          ref={messageInputRef}
          placeholder="메시지를 입력하세요"
          autoComplete="off"
        />
      </form>
      <p className="chat-composer-hint">
        / 슬래쉬 치면 명령을 입력할 수 있습니다.
      </p>
      {commandLoadError && (
        <p className="chat-error" role="alert">
          {commandLoadError}
        </p>
      )}
      {sendError && (
        <p className="chat-error" role="alert">
          {sendError}
        </p>
      )}
    </main>
  );
}
