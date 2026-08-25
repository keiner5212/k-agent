import type { ReactNode, Ref } from "react";

export type TableColumn<TRow> = {
  id: string;
  header: ReactNode;
  className?: string;
  headerClassName?: string;
  cellProps?: (row: TRow) => Record<string, string | undefined>;
  render: (row: TRow) => ReactNode;
};

export type TableProps<TRow> = {
  columns: readonly TableColumn<TRow>[];
  rows: readonly TRow[];
  rowKey: (row: TRow) => string;
  rowTitle?: (row: TRow) => string | undefined;
  className?: string;
  wrapClassName?: string;
  wrapRef?: Ref<HTMLDivElement>;
  scrollable?: boolean;
  layout?: "fixed" | "auto";
  stickyHeader?: boolean;
  cellAlign?: "top" | "middle";
};

const join = (...parts: Array<string | false | undefined>): string =>
  parts.filter(Boolean).join(" ");

export const Table = <TRow,>({
  columns,
  rows,
  rowKey,
  rowTitle,
  className,
  wrapClassName,
  wrapRef,
  scrollable,
  layout = "auto",
  stickyHeader = false,
  cellAlign = "middle",
}: TableProps<TRow>): ReactNode => {
  const table = (
    <table
      className={join(
        "data-table",
        layout === "fixed" && "data-table--fixed",
        stickyHeader && "data-table--sticky",
        cellAlign === "top" && "data-table--top",
        className,
      )}
    >
      <thead>
        <tr>
          {columns.map((column) => (
            <th key={column.id} className={column.headerClassName ?? column.className}>
              {column.header}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => {
          const key = rowKey(row);
          return (
            <tr key={key} title={rowTitle?.(row)}>
              {columns.map((column) => {
                const cellProps = column.cellProps?.(row);
                return (
                  <td key={column.id} className={column.className} {...cellProps}>
                    {column.render(row)}
                  </td>
                );
              })}
            </tr>
          );
        })}
      </tbody>
    </table>
  );

  const needsWrap = Boolean(scrollable);

  if (needsWrap) {
    return (
      <div ref={wrapRef} className={join("data-table-wrap", wrapClassName)}>
        {table}
      </div>
    );
  }

  return table;
};
