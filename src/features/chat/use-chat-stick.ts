import { useCallback, useLayoutEffect, useRef, useState } from "react";
import { useStickToBottom, type StickToBottomInstance } from "use-stick-to-bottom";

const STICK = { resize: "instant", initial: "instant" } as const;
const BOTTOM_PX = 2;

const selectingInside = (scroll: HTMLElement): boolean => {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return false;
  const node = selection.getRangeAt(0).commonAncestorContainer;
  return node.contains(scroll) || scroll.contains(node);
};

const pinBottom = (scroll: HTMLElement): boolean => {
  const next = Math.max(0, scroll.scrollHeight - scroll.clientHeight);
  if (scroll.scrollTop === next) return false;
  scroll.scrollTop = next;
  return true;
};

const markPin = (pinning: { current: boolean }, scroll: HTMLElement): void => {
  if (!pinBottom(scroll)) return;
  pinning.current = true;
  requestAnimationFrame(() => {
    pinning.current = false;
  });
};

const gapFromBottom = (scroll: HTMLElement): number =>
  Math.max(0, scroll.scrollHeight - scroll.clientHeight - scroll.scrollTop);

export type ChatStick = {
  scrollRef: (node: HTMLElement | null) => void;
  contentRef: (node: HTMLElement | null) => void;
  isAtBottom: boolean;
  scrollToBottom: StickToBottomInstance["scrollToBottom"];
};

export const useChatStick = (): ChatStick => {
  const stick = useStickToBottom(STICK);
  const observer = useRef<ResizeObserver | null>(null);
  const follow = useRef(true);
  const pinning = useRef(false);
  const [isAtBottom, setIsAtBottom] = useState(true);
  const { state, scrollToBottom } = stick;
  const libraryScrollRef = stick.scrollRef;
  const libraryContentRef = stick.contentRef;

  const onScroll = useCallback((event: Event) => {
    if (pinning.current) {
      pinning.current = false;
      return;
    }
    const scroll = event.currentTarget;
    if (!(scroll instanceof HTMLElement)) return;
    const next = gapFromBottom(scroll) <= BOTTOM_PX;
    follow.current = next;
    setIsAtBottom((current) => (current === next ? current : next));
  }, []);

  const scrollRef = useCallback(
    (node: HTMLElement | null) => {
      libraryScrollRef.current?.removeEventListener("scroll", onScroll);
      libraryScrollRef(node);
      node?.addEventListener("scroll", onScroll, { passive: true });
    },
    [libraryScrollRef, onScroll],
  );

  const contentRef = useCallback(
    (node: HTMLElement | null) => {
      libraryContentRef(node);
      state.resizeObserver?.disconnect();
      observer.current?.disconnect();
      observer.current = null;
      if (!node) return;
      let lastHeight = 0;
      const next = new ResizeObserver(() => {
        const scroll = libraryScrollRef.current;
        const height = node.offsetHeight;
        const grew = height > lastHeight;
        lastHeight = height;
        if (!scroll || !grew || !follow.current || selectingInside(scroll)) return;
        markPin(pinning, scroll);
      });
      next.observe(node);
      observer.current = next;
    },
    [libraryContentRef, libraryScrollRef, state],
  );

  useLayoutEffect(() => {
    return () => observer.current?.disconnect();
  }, []);

  const jump = useCallback<StickToBottomInstance["scrollToBottom"]>(
    (options) => {
      follow.current = true;
      setIsAtBottom(true);
      const scroll = libraryScrollRef.current;
      if (scroll) markPin(pinning, scroll);
      return scrollToBottom(options);
    },
    [libraryScrollRef, scrollToBottom],
  );

  return { scrollRef, contentRef, isAtBottom, scrollToBottom: jump };
};
