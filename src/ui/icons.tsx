/**
 * 界面里用到的一组线性图标。
 *
 * 全部内联 SVG：图标只有十来个，走字体或图标库都不值得，而且内联的能直接
 * 继承 currentColor，跟深浅色主题自动对上。
 */

type Props = { className?: string };

function svg(path: React.ReactNode) {
  return function Icon({ className }: Props) {
    return (
      <svg
        className={className ? `icon ${className}` : "icon"}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        {path}
      </svg>
    );
  };
}

export const IconSearch = svg(
  <>
    <circle cx="11" cy="11" r="7" />
    <path d="M20 20l-3.6-3.6" />
  </>,
);

export const IconClose = svg(<path d="M6 6l12 12M18 6L6 18" />);

export const IconChevronDown = svg(<path d="M6 9.5l6 6 6-6" />);

export const IconEye = svg(
  <>
    <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" />
    <circle cx="12" cy="12" r="3" />
  </>,
);

/** 同一只眼睛加一道斜杠：轮廓不变，切换时只多/少一条线，16px 下也认得出。 */
export const IconEyeOff = svg(
  <>
    <path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z" />
    <circle cx="12" cy="12" r="3" />
    <path d="M4.5 4.5l15 15" />
  </>,
);

/** 八齿齿轮。轮廓是算出来的多边形，别手改坐标——改一个点整圈就不对称了。 */
export const IconGear = svg(
  <>
    <path d="M9.5 5.0 L10.1 2.2 L13.9 2.2 L14.5 5.0 L15.1 5.3 L17.6 3.7 L20.3 6.4 L18.7 8.9 L19.0 9.5 L21.8 10.1 L21.8 13.9 L19.0 14.5 L18.7 15.1 L20.3 17.6 L17.6 20.3 L15.1 18.7 L14.5 19.0 L13.9 21.8 L10.1 21.8 L9.5 19.0 L8.9 18.7 L6.4 20.3 L3.7 17.6 L5.3 15.1 L5.0 14.5 L2.2 13.9 L2.2 10.1 L5.0 9.5 L5.3 8.9 L3.7 6.4 L6.4 3.7 L8.9 5.3 Z" />
    <circle cx="12" cy="12" r="3.2" />
  </>,
);

export const IconTrash = svg(
  <>
    <path d="M4 7h16" />
    <path d="M10 4h4a1 1 0 011 1v2h-6V5a1 1 0 011-1z" />
    <path d="M6 7l1 12a2 2 0 002 2h6a2 2 0 002-2l1-12" />
    <path d="M10 11v6M14 11v6" />
  </>,
);

export const IconSend = svg(
  <>
    <path d="M21 3L10.5 13.5" />
    <path d="M21 3l-6.8 18-3.7-7.5L3 9.8 21 3z" />
  </>,
);

/** 句子修正 */
export const IconFix = svg(
  <>
    <rect x="3" y="3" width="18" height="18" rx="4" />
    <path d="M8 12.5l2.5 2.5L16 9.5" />
  </>,
);

/** 中文翻译 */
export const IconTranslate = svg(
  <>
    <path d="M3 6h9" />
    <path d="M7.5 4v2c0 3.6-1.7 6.6-4.5 8" />
    <path d="M5 11c1.6 2.4 3.7 4 6.5 5" />
    <path d="M12.5 20l4.2-11 4.3 11" />
    <path d="M14.2 16.2h5.6" />
  </>,
);

/** 语法解析 */
export const IconStructure = svg(
  <>
    <rect x="9" y="3" width="6" height="5" rx="1.5" />
    <rect x="2.5" y="16" width="6" height="5" rx="1.5" />
    <rect x="15.5" y="16" width="6" height="5" rx="1.5" />
    <path d="M12 8v4M5.5 16v-2a2 2 0 012-2h9a2 2 0 012 2v2" />
  </>,
);

/** 关键词解析 */
export const IconKey = svg(
  <>
    <circle cx="8" cy="8" r="4.5" />
    <path d="M11.2 11.2L20 20" />
    <path d="M17 17l-2 2 2 2 2-2" />
  </>,
);

/** 例句 */
export const IconQuote = svg(
  <>
    <path d="M9 7H5.5A2.5 2.5 0 003 9.5V12a2 2 0 002 2h2a2 2 0 002-2V7z" />
    <path d="M9 7v6c0 2.5-1.5 4-4 4" />
    <path d="M21 7h-3.5A2.5 2.5 0 0015 9.5V12a2 2 0 002 2h2a2 2 0 002-2V7z" />
    <path d="M21 7v6c0 2.5-1.5 4-4 4" />
  </>,
);

/** 其他说法 */
export const IconSwap = svg(
  <>
    <path d="M4 8h13l-3-3M20 16H7l3 3" />
  </>,
);

/** 发音。音波只画两道——三道在 16px 下糊成一片实心块。 */
export const IconSpeaker = svg(
  <>
    <path d="M11 5L6.5 8.5H3.5v7h3L11 19V5z" />
    <path d="M14.8 9.6a3.4 3.4 0 010 4.8" />
    <path d="M17.6 6.8a7.4 7.4 0 010 10.4" />
  </>,
);

export const IconWarn = svg(
  <>
    <path d="M12 4.5l8.5 15H3.5L12 4.5z" />
    <path d="M12 10v4M12 17h.01" />
  </>,
);

export const IconCopy = svg(
  <>
    <rect x="9" y="9" width="12" height="12" rx="2.5" />
    <path d="M15 5.5A2.5 2.5 0 0012.5 3h-7A2.5 2.5 0 003 5.5v7A2.5 2.5 0 005.5 15" />
  </>,
);

// 单圈圆形箭头。双弧双箭头在 16px 下笔画挤成一团，认不出是“刷新”。
export const IconRefresh = svg(
  <>
    <path d="M20 12a8 8 0 11-2.34-5.66L20 8.5" />
    <path d="M20 4v4.5h-4.5" />
  </>,
);

export const IconTest = svg(
  <>
    <path d="M9 3h6" />
    <path d="M10 3v5l-5 9a2.5 2.5 0 002.2 3.7h9.6A2.5 2.5 0 0019 17l-5-9V3" />
    <path d="M7.5 15h9" />
  </>,
);

export const IconCheckCircle = svg(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M8 12.5l2.5 2.5L16.5 9" />
  </>,
);

export const IconXCircle = svg(
  <>
    <circle cx="12" cy="12" r="9" />
    <path d="M9 9l6 6M15 9l-6 6" />
  </>,
);

export const IconSpinner = svg(
  <>
    <path d="M12 3a9 9 0 019 9" />
    <path d="M12 21a9 9 0 01-9-9" opacity=".35" />
  </>,
);
