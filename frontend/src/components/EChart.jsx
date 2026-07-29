import { useEffect, useRef } from 'react';
import * as echarts from 'echarts';

// ECharts 暗色主题基础配置
const baseOption = {
  backgroundColor: 'transparent',
  textStyle: { color: '#8a93a6' },
  grid: { left: 60, right: 30, top: 30, bottom: 50 },
  tooltip: {
    trigger: 'axis',
    backgroundColor: '#232b3d',
    borderColor: '#2a3346',
    textStyle: { color: '#e6e6e6' },
  },
  legend: {
    textStyle: { color: '#8a93a6' },
    top: 0,
  },
};

export default function EChart({ option, className = 'chart' }) {
  const ref = useRef(null);
  const chartRef = useRef(null);

  useEffect(() => {
    if (!ref.current) return;
    chartRef.current = echarts.init(ref.current, 'dark');
    chartRef.current.setOption({ ...baseOption, ...option });
    const handleResize = () => chartRef.current && chartRef.current.resize();
    window.addEventListener('resize', handleResize);
    return () => {
      window.removeEventListener('resize', handleResize);
      chartRef.current && chartRef.current.dispose();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (chartRef.current && option) {
      chartRef.current.setOption({ ...baseOption, ...option }, { notMerge: true });
    }
  }, [option]);

  return <div ref={ref} className={className} />;
}
